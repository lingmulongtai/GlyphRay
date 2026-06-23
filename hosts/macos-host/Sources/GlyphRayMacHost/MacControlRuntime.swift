import Foundation
import CryptoKit

#if canImport(CoreGraphics)
import CoreGraphics
#endif

#if canImport(Network)
import Network
#endif

private let macControlDefaultPort: UInt16 = 44_999
private let macTrustedAuthChallengeTTLMS: UInt64 = 30_000
private let macPairingCodeTTLMS: UInt64 = 5 * 60_000
private let macPairingChallengeTTLMS: UInt64 = 2 * 60_000
private let macMaxPairingCodeAttempts = 5
private let macPairingAttemptWindowMS: UInt64 = 2 * 60_000

struct MacPairingClient: Identifiable, Equatable, Codable {
    let id: String
    let deviceName: String
    let target: MacUdpSendTarget
    let publicKeyFingerprint: String?
    let publicKeyDER: Data?
    let pairedAtUnixMs: UInt64

    var publicKeyStatusLabel: String {
        publicKeyFingerprint == nil ? "none" : "sha256"
    }
}

struct MacClientVideoPreference: Equatable, Codable {
    let displayID: UInt32
    let codec: UInt32
    let colorSpace: UInt32
    let width: UInt32
    let height: UInt32
    let maxFPS: UInt16
    let targetBitrateKbps: UInt32
    let keyframeIntervalMs: UInt32
    let lowLatency: Bool
}

struct MacControlRuntimeSnapshot: Equatable {
    let listening: Bool
    let bindPort: UInt16
    let pairingRequestsReceived: UInt64
    let acceptedClients: [MacPairingClient]
    let lastApprovedTarget: MacUdpSendTarget?
    let secureTargets: [MacUdpSendTarget]
    let lastVideoPreference: MacClientVideoPreference?
    let pendingAuthChallenges: Int
    let pendingPairingChallenges: Int
    let pairingCode: String?
    let pendingKeyExchanges: Int
    let secureClients: Int
    let hostIdentityFingerprint: String?
    let mouseEventsInjected: UInt64
    let keyboardEventsInjected: UInt64
    let touchBatchesInjected: UInt64
    let lastEvent: String
}

private struct MacPendingAuthChallenge {
    let challengeID: UInt64
    let nonce: Data
    let issuedAtUnixMs: UInt64
    let expectedDeviceID: String
    let publicKeyDER: Data
    let deviceName: String
}

private struct MacPendingPairingChallenge {
    let salt: Data
    let expiresAtUnixMs: UInt64
}

final class MacControlRuntime {
    var onSnapshot: ((MacControlRuntimeSnapshot) -> Void)?

    private let trustedClientStore: MacTrustedClientStore
    private let hostIdentity: MacHostIdentity?
    private let inputController = InputEventController()
    private let queue = DispatchQueue(label: "com.glyphray.mac.control-runtime")
    private var bindPort: UInt16 = macControlDefaultPort
    private var pairingRequestsReceived: UInt64 = 0
    private var acceptedClients: [MacPairingClient] = []
    private var lastApprovedTarget: MacUdpSendTarget?
    private var lastVideoPreference: MacClientVideoPreference?
    private var pendingAuthChallenges: [String: MacPendingAuthChallenge] = [:]
    private var pendingPairingChallenges: [String: MacPendingPairingChallenge] = [:]
    private var pairingCodeAttempts: [String: Int] = [:]
    private var pairingAttemptWindowStarted: [String: UInt64] = [:]
    private var pairingCode = MacOneTimePairingCode.generate()
    private var pairingCodeExpiresAtUnixMs = currentUnixMilliseconds() + macPairingCodeTTLMS
    private var pendingKeyExchanges: [String: MacPendingKeyExchange] = [:]
    private var secureSessions: [String: MacSecureSessionCodec] = [:]
    private var lastInputSequences: [String: UInt64] = [:]
    private var mouseEventsInjected: UInt64 = 0
    private var keyboardEventsInjected: UInt64 = 0
    private var touchBatchesInjected: UInt64 = 0
    private var lastEvent = "Control runtime idle"

    #if canImport(Network)
    private var listener: NWListener?
    private var connections: [NWConnection] = []
    #endif

    init(trustedClientStore: MacTrustedClientStore = MacTrustedClientStore()) {
        self.trustedClientStore = trustedClientStore
        let identityRecoveryEvent: String?
        do {
            let loaded = try MacHostIdentityStore().loadOrRecover()
            hostIdentity = loaded.identity
            identityRecoveryEvent = loaded.quarantinedAccount.map {
                "Host identity was corrupt and quarantined as \($0); clients must approve the new fingerprint"
            }
        } catch {
            hostIdentity = nil
            identityRecoveryEvent = nil
            lastEvent = "Host identity load failed: \(error)"
        }
        do {
            let loadedClients = try trustedClientStore.loadOrRecover()
            acceptedClients = loadedClients.clients
            lastApprovedTarget = acceptedClients.first?.target
            if hostIdentity == nil {
                lastEvent = "Host identity unavailable; pairing is disabled"
            } else if let identityRecoveryEvent {
                lastEvent = identityRecoveryEvent
            } else if let quarantine = loadedClients.quarantinedAccount {
                lastEvent = "Trusted clients were corrupt and quarantined as \(quarantine); pairing approval is required again"
            } else if acceptedClients.isEmpty {
                lastEvent = "Control runtime idle"
            } else {
                lastEvent = "Loaded \(acceptedClients.count) trusted macOS client(s)"
            }
        } catch {
            lastEvent = "Trusted client load failed: \(error)"
        }
    }

    func start(port: UInt16 = macControlDefaultPort) throws {
        #if canImport(Network)
        guard listener == nil else {
            publishSnapshot()
            return
        }
        guard let nwPort = NWEndpoint.Port(rawValue: port) else {
            throw MacHostError.transportUnavailable("Invalid control port \(port)")
        }

        let listener = try NWListener(using: .udp, on: nwPort)
        bindPort = port
        lastEvent = "Starting control runtime on UDP \(port)"
        listener.stateUpdateHandler = { [weak self] state in
            self?.queue.async {
                self?.handleListenerState(state)
            }
        }
        listener.newConnectionHandler = { [weak self] connection in
            self?.queue.async {
                self?.startConnection(connection)
            }
        }
        self.listener = listener
        listener.start(queue: queue)
        publishSnapshot()
        #else
        throw MacHostError.frameworkUnavailable("Network")
        #endif
    }

    func stop() {
        #if canImport(Network)
        queue.async {
            self.listener?.cancel()
            self.listener = nil
            for connection in self.connections {
                connection.cancel()
            }
            self.connections.removeAll(keepingCapacity: false)
            self.pendingKeyExchanges.removeAll()
            self.pendingPairingChallenges.removeAll()
            self.pairingCodeAttempts.removeAll()
            self.pairingAttemptWindowStarted.removeAll()
            self.secureSessions.removeAll()
            self.lastInputSequences.removeAll()
            self.lastEvent = "Control runtime stopped"
            self.publishSnapshot()
        }
        #endif
    }

    func snapshot() -> MacControlRuntimeSnapshot {
        queue.sync {
            makeSnapshot()
        }
    }

    func hasSecureSession(for target: MacUdpSendTarget) -> Bool {
        queue.sync { secureSessions[target.storageKey] != nil }
    }

    func preferredSecureTarget() -> MacUdpSendTarget? {
        queue.sync {
            if let lastApprovedTarget,
               secureSessions[lastApprovedTarget.storageKey] != nil {
                return lastApprovedTarget
            }
            return acceptedClients.first { client in
                secureSessions[client.target.storageKey] != nil
            }?.target
        }
    }

    func sealVideoDatagram(_ datagram: Data, for target: MacUdpSendTarget) throws -> Data {
        try queue.sync {
            guard let codec = secureSessions[target.storageKey] else {
                throw MacSecureSessionError.missingSession
            }
            return try codec.seal(datagram)
        }
    }

    func clearTrustedClients() {
        queue.async {
            do {
                try self.trustedClientStore.clear()
                self.acceptedClients.removeAll()
                self.pendingAuthChallenges.removeAll()
                self.pendingKeyExchanges.removeAll()
                self.secureSessions.removeAll()
                self.lastInputSequences.removeAll()
                self.lastApprovedTarget = nil
                self.lastEvent = "Trusted macOS clients cleared"
            } catch {
                self.lastEvent = "Trusted client clear failed: \(error)"
            }
            self.publishSnapshot()
        }
    }

    #if canImport(Network)
    private func handleListenerState(_ state: NWListener.State) {
        switch state {
        case .ready:
            lastEvent = "Control runtime listening on UDP \(bindPort)"
        case .failed(let error):
            lastEvent = "Control runtime failed: \(error)"
        case .cancelled:
            lastEvent = "Control runtime stopped"
        default:
            break
        }
        publishSnapshot()
    }

    private func startConnection(_ connection: NWConnection) {
        connections.append(connection)
        connection.stateUpdateHandler = { [weak self] state in
            switch state {
            case .failed(let error):
                self?.queue.async {
                    self?.dropConnectionState(for: connection, reason: "Control peer failed: \(error)")
                }
            case .cancelled:
                self?.queue.async {
                    self?.dropConnectionState(for: connection, reason: "Control peer disconnected")
                }
            default:
                break
            }
        }
        connection.start(queue: queue)
        receiveNextMessage(on: connection)
    }

    private func receiveNextMessage(on connection: NWConnection) {
        connection.receiveMessage { [weak self, weak connection] content, _, _, error in
            guard let self, let connection else {
                return
            }
            self.queue.async {
                if let error {
                    self.lastEvent = "Control receive failed: \(error)"
                    self.publishSnapshot()
                    return
                }
                if let content {
                    self.handleDatagram(content, from: connection)
                }
                self.receiveNextMessage(on: connection)
            }
        }
    }

    private func dropConnectionState(for connection: NWConnection, reason: String) {
        if let target = MacUdpSendTarget(endpoint: connection.endpoint) {
            let key = target.storageKey
            pendingAuthChallenges.removeValue(forKey: key)
            pendingPairingChallenges.removeValue(forKey: key)
            pendingKeyExchanges.removeValue(forKey: key)
            secureSessions.removeValue(forKey: key)
            lastInputSequences.removeValue(forKey: key)
            if lastApprovedTarget == target {
                lastApprovedTarget = acceptedClients.first { client in
                    secureSessions[client.target.storageKey] != nil
                }?.target
            }
        }
        connections.removeAll { $0 === connection }
        lastEvent = reason
        publishSnapshot()
    }

    private func handleDatagram(_ data: Data, from connection: NWConnection) {
        guard let target = MacUdpSendTarget(endpoint: connection.endpoint) else {
            return
        }
        let storageKey = target.storageKey
        let plaintext: Data
        do {
            if data.starts(with: Data("GLYE".utf8)) {
                guard let codec = secureSessions[storageKey] else {
                    throw MacSecureSessionError.missingSession
                }
                plaintext = try codec.open(data)
            } else {
                guard secureSessions[storageKey] == nil else {
                    throw MacSecureSessionError.invalidPacket(
                        "plaintext rejected after secure-session establishment"
                    )
                }
                plaintext = data
            }
        } catch {
            lastEvent = "Rejected control datagram: \(error)"
            publishSnapshot()
            return
        }

        guard let packet = try? MacTransportDatagram.decode(plaintext) else {
            return
        }
        if packet.channel == .input {
            handleInputPacket(packet, target: target)
            return
        }
        guard packet.channel == .control else { return }
        if packet.messageKind == .sessionKeyConfirm {
            handleSessionKeyConfirm(packet: packet, target: target, connection: connection)
            return
        }
        guard let frame = try? MacProtocolFrame.decode(packet.payload) else {
            lastEvent = "Ignored malformed control frame"
            publishSnapshot()
            return
        }

        switch frame.messageKind {
        case .authResponse:
            handleAuthResponse(frame: frame, connection: connection)
        case .pairingRequest:
            handlePairingRequest(frame: frame, connection: connection)
        case .encoderConfig:
            guard secureSessions[storageKey] != nil else {
                lastEvent = "Rejected plaintext encoder configuration before secure session"
                publishSnapshot()
                return
            }
            if let preference = try? MacClientVideoPreference.decode(frame.payload) {
                lastVideoPreference = preference
                lastEvent = "Client requested \(preference.width)x\(preference.height) @ \(preference.maxFPS)fps"
                publishSnapshot()
            }
        case .latencyPing:
            guard secureSessions[storageKey] != nil else {
                lastEvent = "Rejected plaintext latency request before secure session"
                publishSnapshot()
                return
            }
            handleLatencyPing(frame: frame, target: target, connection: connection)
        default:
            lastEvent = "Ignored control message kind \(frame.messageKind.rawValue)"
            publishSnapshot()
        }
    }

    private func handlePairingRequest(frame: MacProtocolFrame, connection: NWConnection) {
        guard
            let request = try? MacPairingRequest.decode(frame.payload),
            let target = MacUdpSendTarget(endpoint: connection.endpoint)
        else {
            lastEvent = "Ignored malformed pairing request"
            publishSnapshot()
            return
        }

        pairingRequestsReceived += 1
        guard !request.oneTimePublicKey.isEmpty else {
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "device identity key is required",
                frameSequence: frame.sequence,
                connection: connection
            )
            lastEvent = "Rejected pairing without a device identity key"
            publishSnapshot()
            return
        }
        guard hostIdentity != nil else {
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "host identity is unavailable",
                frameSequence: frame.sequence,
                connection: connection
            )
            lastEvent = "Rejected pairing because host identity is unavailable"
            publishSnapshot()
            return
        }
        let publicKeyDER = request.oneTimePublicKey
        let fingerprint = MacTrustedIdentity.publicKeyFingerprint(publicKeyDER)
        let trustedID = MacTrustedIdentity.trustedDeviceID(forPublicKeyDER: publicKeyDER)

        if let existing = acceptedClients.first(where: {
            $0.publicKeyFingerprint == fingerprint && $0.publicKeyDER != nil
        }),
           let storedPublicKeyDER = existing.publicKeyDER {
            queueTrustedAuthChallenge(
                expectedDeviceID: existing.id,
                publicKeyDER: storedPublicKeyDER,
                deviceName: request.deviceName,
                target: target,
                frameSequence: frame.sequence,
                connection: connection
            )
            return
        }

        if request.pairingCodeHash.isEmpty {
            queuePairingCodeChallenge(
                target: target,
                frameSequence: frame.sequence,
                connection: connection
            )
            return
        }
        guard verifyPairingCode(
            proof: request.pairingCodeHash,
            target: target
        ) else {
            let attempts = pairingCodeAttempts[target.storageKey, default: 0]
            let remaining = max(0, macMaxPairingCodeAttempts - attempts)
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "pairing code was invalid or expired; \(remaining) attempt(s) remain",
                frameSequence: frame.sequence,
                connection: connection
            )
            lastEvent = "Rejected invalid pairing code from \(request.deviceName)"
            publishSnapshot()
            return
        }

        let client = MacPairingClient(
            id: trustedID,
            deviceName: request.deviceName,
            target: target,
            publicKeyFingerprint: fingerprint,
            publicKeyDER: publicKeyDER,
            pairedAtUnixMs: currentUnixMilliseconds()
        )

        acceptTrustedClient(
            client,
            successEvent: "Accepted \(request.deviceName) at \(target.host):\(target.port)"
        )
        sendPairingResult(
            accepted: true,
            trustedDeviceID: trustedID,
            reason: nil,
            frameSequence: frame.sequence,
            connection: connection
        )
        queueSessionKeyExchange(
            client: client,
            frameSequence: frame.sequence,
            connection: connection
        )
        publishSnapshot()
    }

    private func handleAuthResponse(frame: MacProtocolFrame, connection: NWConnection) {
        guard
            let response = try? MacAuthResponse.decode(frame.payload),
            let target = MacUdpSendTarget(endpoint: connection.endpoint)
        else {
            lastEvent = "Ignored malformed auth response"
            publishSnapshot()
            return
        }

        let challengeKey = target.storageKey
        guard let pending = pendingAuthChallenges.removeValue(forKey: challengeKey) else {
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "auth response arrived without a pending challenge",
                frameSequence: frame.sequence,
                connection: connection
            )
            lastEvent = "Rejected auth response without pending challenge"
            publishSnapshot()
            return
        }

        let reject: (String) -> Void = { reason in
            self.sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: reason,
                frameSequence: frame.sequence,
                connection: connection
            )
            self.lastEvent = "Trusted auth rejected: \(reason)"
            self.publishSnapshot()
        }

        guard response.challengeID == pending.challengeID else {
            reject("auth response challenge id did not match")
            return
        }
        guard response.deviceID == pending.expectedDeviceID else {
            reject("auth response device id did not match")
            return
        }
        let nowMs = currentUnixMilliseconds()
        let challengeAgeMs = nowMs >= pending.issuedAtUnixMs ? nowMs - pending.issuedAtUnixMs : 0
        guard challengeAgeMs <= macTrustedAuthChallengeTTLMS else {
            reject("auth challenge expired")
            return
        }

        do {
            try MacTrustedIdentity.verifyTrustedSignature(
                publicKeyDER: pending.publicKeyDER,
                trustedDeviceID: pending.expectedDeviceID,
                challengeID: pending.challengeID,
                nonce: pending.nonce,
                signatureDER: response.signature
            )
        } catch {
            reject("\(error)")
            return
        }

        let client = MacPairingClient(
            id: pending.expectedDeviceID,
            deviceName: pending.deviceName,
            target: target,
            publicKeyFingerprint: MacTrustedIdentity.publicKeyFingerprint(pending.publicKeyDER),
            publicKeyDER: pending.publicKeyDER,
            pairedAtUnixMs: currentUnixMilliseconds()
        )
        acceptTrustedClient(
            client,
            successEvent: "Trusted client authenticated: \(pending.deviceName)"
        )
        sendPairingResult(
            accepted: true,
            trustedDeviceID: pending.expectedDeviceID,
            reason: nil,
            frameSequence: frame.sequence,
            connection: connection
        )
        queueSessionKeyExchange(
            client: client,
            frameSequence: frame.sequence,
            connection: connection
        )
        publishSnapshot()
    }

    private func queueSessionKeyExchange(
        client: MacPairingClient,
        frameSequence: UInt64,
        connection: NWConnection
    ) {
        guard let hostIdentity, let clientIdentity = client.publicKeyDER else {
            lastEvent = "Secure-session key exchange unavailable for \(client.deviceName)"
            return
        }
        do {
            let (pending, payload) = try MacSecureSessionHandshake.begin(
                hostIdentity: hostIdentity,
                expectedDeviceID: client.id,
                clientIdentityPublicKeyDER: clientIdentity,
                nowUnixMs: currentUnixMilliseconds()
            )
            pendingKeyExchanges[client.target.storageKey] = pending
            secureSessions.removeValue(forKey: client.target.storageKey)
            let datagram = MacTransportDatagram.encode(
                channel: .control,
                messageKind: .sessionKeyExchange,
                sequence: frameSequence,
                timestampUs: monotonicMicroseconds(),
                payload: payload
            )
            connection.send(content: datagram, completion: .contentProcessed { _ in })
            lastEvent = "Secure-session offer sent to \(client.deviceName)"
        } catch {
            pendingKeyExchanges.removeValue(forKey: client.target.storageKey)
            lastEvent = "Secure-session offer failed: \(error)"
        }
    }

    private func handleSessionKeyConfirm(
        packet: MacTransportDatagram,
        target: MacUdpSendTarget,
        connection: NWConnection
    ) {
        let key = target.storageKey
        guard let pending = pendingKeyExchanges[key] else {
            lastEvent = "Rejected unexpected secure-session confirmation"
            publishSnapshot()
            return
        }
        do {
            let codec = try MacSecureSessionHandshake.finish(
                pending: pending,
                encodedConfirm: packet.payload,
                nowUnixMs: currentUnixMilliseconds()
            )
            pendingKeyExchanges.removeValue(forKey: key)
            secureSessions[key] = codec
            lastInputSequences.removeValue(forKey: key)
            lastApprovedTarget = target
            try sendDisplayInfo(
                target: target,
                frameSequence: packet.sequence,
                connection: connection
            )
            lastEvent = "Encrypted session established; display info sent to \(target.host):\(target.port)"
            publishSnapshot()
        } catch {
            pendingKeyExchanges.removeValue(forKey: key)
            secureSessions.removeValue(forKey: key)
            lastEvent = "Secure-session confirmation rejected: \(error)"
            connection.cancel()
            publishSnapshot()
        }
    }

    private func handleInputPacket(_ packet: MacTransportDatagram, target: MacUdpSendTarget) {
        let key = target.storageKey
        guard secureSessions[key] != nil else {
            lastEvent = "Rejected input before encrypted session establishment"
            publishSnapshot()
            return
        }
        guard lastInputSequences[key].map({ packet.sequence > $0 }) ?? true else {
            lastEvent = "Dropped stale input sequence from \(target.host)"
            publishSnapshot()
            return
        }
        do {
            let frame = try MacProtocolFrame.decode(packet.payload)
            guard frame.messageKind == packet.messageKind else {
                throw MacHostError.transportUnavailable("Input message kind mismatch")
            }
            switch frame.messageKind {
            case .mouseInput:
                let input = try MacRemoteInputDecoder.decodeMouse(frame.payload)
                inputController.postMouse(event: RemotePointerEvent(
                    x: Double(input.x),
                    y: Double(input.y),
                    wheelDeltaX: Double(input.wheelDeltaX),
                    wheelDeltaY: Double(input.wheelDeltaY),
                    buttonFlags: input.buttonFlags
                ))
                mouseEventsInjected += 1
                lastEvent = "Injected remote mouse input"
            case .keyboardInput:
                if inputController.postKeyboard(
                    event: try MacRemoteInputDecoder.decodeKeyboard(frame.payload)
                ) {
                    keyboardEventsInjected += 1
                    lastEvent = "Injected remote keyboard input"
                } else {
                    lastEvent = "Ignored unmapped remote keyboard key"
                }
            case .touchInputBatch:
                inputController.postTouch(
                    batch: try MacRemoteInputDecoder.decodeTouchBatch(frame.payload)
                )
                touchBatchesInjected += 1
                lastEvent = "Injected remote touch as pointer input"
            default:
                lastEvent = "Input kind \(frame.messageKind.rawValue) is not available on macOS"
            }
            lastInputSequences[key] = packet.sequence
        } catch {
            lastEvent = "Rejected malformed remote input: \(error)"
        }
        publishSnapshot()
    }

    private func sendDisplayInfo(
        target: MacUdpSendTarget,
        frameSequence: UInt64,
        connection: NWConnection
    ) throws {
        guard let codec = secureSessions[target.storageKey] else {
            throw MacSecureSessionError.missingSession
        }
        let frame = MacProtocolFrame.encode(
            sequence: frameSequence,
            messageKind: .displayInfo,
            payload: try MacDisplayInfo.encodeActiveDisplays()
        )
        let datagram = MacTransportDatagram.encode(
            channel: .control,
            messageKind: .displayInfo,
            sequence: frameSequence,
            timestampUs: monotonicMicroseconds(),
            payload: frame
        )
        connection.send(content: try codec.seal(datagram), completion: .contentProcessed { _ in })
    }

    private func queueTrustedAuthChallenge(
        expectedDeviceID: String,
        publicKeyDER: Data,
        deviceName: String,
        target: MacUdpSendTarget,
        frameSequence: UInt64,
        connection: NWConnection
    ) {
        do {
            let challenge = MacPendingAuthChallenge(
                challengeID: try MacTrustedIdentity.makeChallengeID(),
                nonce: try MacTrustedIdentity.makeChallengeNonce(),
                issuedAtUnixMs: currentUnixMilliseconds(),
                expectedDeviceID: expectedDeviceID,
                publicKeyDER: publicKeyDER,
                deviceName: deviceName
            )
            pendingAuthChallenges[target.storageKey] = challenge
            let responseFrame = MacProtocolFrame.encode(
                sequence: frameSequence,
                messageKind: .authChallenge,
                payload: MacAuthChallenge.encode(challenge)
            )
            let responseDatagram = MacTransportDatagram.encode(
                channel: .control,
                messageKind: .authChallenge,
                sequence: frameSequence,
                timestampUs: monotonicMicroseconds(),
                payload: responseFrame
            )
            connection.send(content: responseDatagram, completion: .contentProcessed { _ in })
            lastEvent = "Trusted client matched; auth challenge sent to \(deviceName)"
        } catch {
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "\(error)",
                frameSequence: frameSequence,
                connection: connection
            )
            lastEvent = "Trusted auth challenge failed: \(error)"
        }
        publishSnapshot()
    }

    private func acceptTrustedClient(_ client: MacPairingClient, successEvent: String) {
        acceptedClients.removeAll { $0.id == client.id || $0.target == client.target }
        acceptedClients.insert(client, at: 0)
        if acceptedClients.count > 8 {
            acceptedClients.removeLast(acceptedClients.count - 8)
        }
        lastApprovedTarget = client.target
        do {
            try trustedClientStore.save(acceptedClients)
            lastEvent = successEvent
        } catch {
            lastEvent = "\(successEvent), trusted client save failed: \(error)"
        }
    }

    private func queuePairingCodeChallenge(
        target: MacUdpSendTarget,
        frameSequence: UInt64,
        connection: NWConnection
    ) {
        let now = currentUnixMilliseconds()
        if now >= pairingCodeExpiresAtUnixMs {
            rotatePairingCode()
        }
        let key = target.storageKey
        refreshPairingAttemptWindow(key: key, now: now)
        guard pairingCodeAttempts[key, default: 0] < macMaxPairingCodeAttempts else {
            sendPairingResult(
                accepted: false,
                trustedDeviceID: nil,
                reason: "pairing code attempt limit reached",
                frameSequence: frameSequence,
                connection: connection
            )
            lastEvent = "Pairing attempt limit reached for \(target.host)"
            publishSnapshot()
            return
        }

        let salt = MacOneTimePairingCode.randomBytes(count: 32)
        let expiresAtUnixMs = min(now + macPairingChallengeTTLMS, pairingCodeExpiresAtUnixMs)
        pendingPairingChallenges[key] = MacPendingPairingChallenge(
            salt: salt,
            expiresAtUnixMs: expiresAtUnixMs
        )
        let frame = MacProtocolFrame.encode(
            sequence: frameSequence,
            messageKind: .pairingChallenge,
            payload: MacPairingChallenge.encode(
                salt: salt,
                expiresAtUnixMs: expiresAtUnixMs
            )
        )
        let datagram = MacTransportDatagram.encode(
            channel: .control,
            messageKind: .pairingChallenge,
            sequence: frameSequence,
            timestampUs: monotonicMicroseconds(),
            payload: frame
        )
        connection.send(content: datagram, completion: .contentProcessed { _ in })
        lastEvent = "Pairing code required for \(target.host)"
        publishSnapshot()
    }

    private func verifyPairingCode(proof: Data, target: MacUdpSendTarget) -> Bool {
        let key = target.storageKey
        let now = currentUnixMilliseconds()
        refreshPairingAttemptWindow(key: key, now: now)
        guard pairingCodeAttempts[key, default: 0] < macMaxPairingCodeAttempts else {
            return false
        }
        guard let challenge = pendingPairingChallenges.removeValue(forKey: key) else {
            pairingCodeAttempts[key, default: 0] += 1
            return false
        }
        guard now <= challenge.expiresAtUnixMs, now <= pairingCodeExpiresAtUnixMs else {
            pairingCodeAttempts[key, default: 0] += 1
            return false
        }
        guard MacPairingCodeProof.verify(
            code: pairingCode,
            salt: challenge.salt,
            proof: proof
        ) else {
            pairingCodeAttempts[key, default: 0] += 1
            return false
        }

        pairingCodeAttempts.removeValue(forKey: key)
        pairingAttemptWindowStarted.removeValue(forKey: key)
        rotatePairingCode()
        return true
    }

    private func refreshPairingAttemptWindow(key: String, now: UInt64) {
        guard let started = pairingAttemptWindowStarted[key] else {
            pairingAttemptWindowStarted[key] = now
            return
        }
        if now >= started, now - started >= macPairingAttemptWindowMS {
            pairingCodeAttempts[key] = 0
            pairingAttemptWindowStarted[key] = now
        }
    }

    private func rotatePairingCode() {
        pairingCode = MacOneTimePairingCode.generate()
        pairingCodeExpiresAtUnixMs = currentUnixMilliseconds() + macPairingCodeTTLMS
        pendingPairingChallenges.removeAll()
    }

    private func sendPairingResult(
        accepted: Bool,
        trustedDeviceID: String?,
        reason: String?,
        frameSequence: UInt64,
        connection: NWConnection
    ) {
        let responseFrame = MacProtocolFrame.encode(
            sequence: frameSequence,
            messageKind: .pairingResult,
            payload: MacPairingResult.encode(
                accepted: accepted,
                trustedDeviceID: trustedDeviceID,
                reason: reason
            )
        )
        let responseDatagram = MacTransportDatagram.encode(
            channel: .control,
            messageKind: .pairingResult,
            sequence: frameSequence,
            timestampUs: monotonicMicroseconds(),
            payload: responseFrame
        )
        connection.send(content: responseDatagram, completion: .contentProcessed { _ in })
    }

    private func handleLatencyPing(
        frame: MacProtocolFrame,
        target: MacUdpSendTarget,
        connection: NWConnection
    ) {
        guard let ping = try? MacLatencyPing.decode(frame.payload) else {
            return
        }
        let hostReceiveUs = monotonicMicroseconds()
        let hostSendUs = monotonicMicroseconds()
        let pongFrame = MacProtocolFrame.encode(
            sequence: frame.sequence,
            messageKind: .latencyPong,
            payload: MacLatencyPong.encode(
                sequence: ping.sequence,
                clientSendTimestampUs: ping.clientSendTimestampUs,
                hostReceiveTimestampUs: hostReceiveUs,
                hostSendTimestampUs: hostSendUs
            )
        )
        let datagram = MacTransportDatagram.encode(
            channel: .control,
            messageKind: .latencyPong,
            sequence: frame.sequence,
            timestampUs: hostSendUs,
            payload: pongFrame
        )
        do {
            guard let codec = secureSessions[target.storageKey] else {
                throw MacSecureSessionError.missingSession
            }
            let outgoing = try codec.seal(datagram)
            connection.send(content: outgoing, completion: .contentProcessed { _ in })
        } catch {
            lastEvent = "Latency pong encryption failed: \(error)"
            publishSnapshot()
            return
        }
        lastEvent = "Latency pong sent"
        publishSnapshot()
    }
    #endif

    private func makeSnapshot() -> MacControlRuntimeSnapshot {
        #if canImport(Network)
        let isListening = listener != nil
        #else
        let isListening = false
        #endif
        return MacControlRuntimeSnapshot(
            listening: isListening,
            bindPort: bindPort,
            pairingRequestsReceived: pairingRequestsReceived,
            acceptedClients: acceptedClients,
            lastApprovedTarget: lastApprovedTarget,
            secureTargets: acceptedClients.compactMap { client in
                secureSessions[client.target.storageKey] == nil ? nil : client.target
            },
            lastVideoPreference: lastVideoPreference,
            pendingAuthChallenges: pendingAuthChallenges.count,
            pendingPairingChallenges: pendingPairingChallenges.count,
            pairingCode: pendingPairingChallenges.isEmpty ? nil : pairingCode,
            pendingKeyExchanges: pendingKeyExchanges.count,
            secureClients: secureSessions.count,
            hostIdentityFingerprint: hostIdentity?.fingerprint,
            mouseEventsInjected: mouseEventsInjected,
            keyboardEventsInjected: keyboardEventsInjected,
            touchBatchesInjected: touchBatchesInjected,
            lastEvent: lastEvent
        )
    }

    private func publishSnapshot() {
        onSnapshot?(makeSnapshot())
    }
}

private enum MacTransportChannel: UInt8 {
    case video = 1
    case audio = 2
    case input = 3
    case control = 4
}

private enum MacControlMessageKind: UInt16 {
    case authChallenge = 3
    case authResponse = 4
    case pairingRequest = 5
    case pairingResult = 6
    case displayInfo = 7
    case encoderConfig = 8
    case videoFrame = 9
    case stylusInputBatch = 11
    case mouseInput = 12
    case keyboardInput = 13
    case latencyPing = 15
    case latencyPong = 16
    case touchInputBatch = 19
    case gamepadInput = 20
    case sessionKeyExchange = 21
    case sessionKeyConfirm = 22
    case pairingChallenge = 23
}

private struct MacTransportDatagram {
    let channel: MacTransportChannel
    let messageKind: MacControlMessageKind
    let sequence: UInt64
    let timestampUs: UInt64
    let payload: Data

    static func decode(_ data: Data) throws -> MacTransportDatagram {
        var reader = MacBinaryReader(data)
        guard try reader.readBytes(count: 4) == Data([0x47, 0x4c, 0x59, 0x54]) else {
            throw MacHostError.transportUnavailable("Invalid transport magic")
        }
        guard try reader.readUInt16() == 1 else {
            throw MacHostError.transportUnavailable("Unsupported transport version")
        }
        guard let channel = MacTransportChannel(rawValue: try reader.readUInt8()) else {
            throw MacHostError.transportUnavailable("Unknown transport channel")
        }
        guard let messageKind = MacControlMessageKind(rawValue: try reader.readUInt16()) else {
            throw MacHostError.transportUnavailable("Unknown control message kind")
        }
        let sequence = try reader.readUInt64()
        let timestampUs = try reader.readUInt64()
        let payloadLength = Int(try reader.readUInt32())
        let expectedCRC = try reader.readUInt32()
        let payload = try reader.readBytes(count: payloadLength)
        guard reader.isAtEnd, macControlCRC32(payload) == expectedCRC else {
            throw MacHostError.transportUnavailable("Transport checksum mismatch")
        }
        return MacTransportDatagram(
            channel: channel,
            messageKind: messageKind,
            sequence: sequence,
            timestampUs: timestampUs,
            payload: payload
        )
    }

    static func encode(
        channel: MacTransportChannel,
        messageKind: MacControlMessageKind,
        sequence: UInt64,
        timestampUs: UInt64,
        payload: Data
    ) -> Data {
        var out = Data(capacity: 33 + payload.count)
        out.append(contentsOf: [0x47, 0x4c, 0x59, 0x54])
        out.appendLittleEndian(UInt16(1))
        out.append(channel.rawValue)
        out.appendLittleEndian(messageKind.rawValue)
        out.appendLittleEndian(sequence)
        out.appendLittleEndian(timestampUs)
        out.appendLittleEndian(UInt32(payload.count))
        out.appendLittleEndian(macControlCRC32(payload))
        out.append(payload)
        return out
    }
}

private struct MacProtocolFrame {
    let sequence: UInt64
    let messageKind: MacControlMessageKind
    let payload: Data

    static func decode(_ data: Data) throws -> MacProtocolFrame {
        var reader = MacBinaryReader(data)
        guard try reader.readBytes(count: 4) == Data([0x47, 0x4c, 0x59, 0x52]) else {
            throw MacHostError.transportUnavailable("Invalid protocol magic")
        }
        guard try reader.readUInt16() == 1 else {
            throw MacHostError.transportUnavailable("Unsupported protocol version")
        }
        guard let messageKind = MacControlMessageKind(rawValue: try reader.readUInt16()) else {
            throw MacHostError.transportUnavailable("Unknown protocol message kind")
        }
        let sequence = try reader.readUInt64()
        let payloadLength = Int(try reader.readUInt32())
        let expectedCRC = try reader.readUInt32()
        let payload = try reader.readBytes(count: payloadLength)
        guard reader.isAtEnd, macControlCRC32(payload) == expectedCRC else {
            throw MacHostError.transportUnavailable("Protocol checksum mismatch")
        }
        return MacProtocolFrame(sequence: sequence, messageKind: messageKind, payload: payload)
    }

    static func encode(sequence: UInt64, messageKind: MacControlMessageKind, payload: Data) -> Data {
        var out = Data(capacity: 24 + payload.count)
        out.append(contentsOf: [0x47, 0x4c, 0x59, 0x52])
        out.appendLittleEndian(UInt16(1))
        out.appendLittleEndian(messageKind.rawValue)
        out.appendLittleEndian(sequence)
        out.appendLittleEndian(UInt32(payload.count))
        out.appendLittleEndian(macControlCRC32(payload))
        out.append(payload)
        return out
    }
}

private struct MacPairingRequest {
    let deviceName: String
    let pairingCodeHash: Data
    let oneTimePublicKey: Data

    static func decode(_ payload: Data) throws -> MacPairingRequest {
        var reader = MacBinaryReader(payload)
        guard try reader.readUInt32() == 4 else {
            throw MacHostError.transportUnavailable("Payload did not contain PairingRequest")
        }
        let deviceName = try reader.readBincodeString()
        let pairingCodeHash = try reader.readBincodeBytes()
        let oneTimePublicKey = try reader.readBincodeBytes()
        guard reader.isAtEnd else {
            throw MacHostError.transportUnavailable("PairingRequest contained trailing bytes")
        }
        return MacPairingRequest(
            deviceName: deviceName,
            pairingCodeHash: pairingCodeHash,
            oneTimePublicKey: oneTimePublicKey
        )
    }
}

private enum MacAuthChallenge {
    static func encode(_ challenge: MacPendingAuthChallenge) -> Data {
        var out = Data()
        out.appendLittleEndian(UInt32(2))
        out.appendLittleEndian(challenge.challengeID)
        out.append(challenge.nonce)
        out.appendLittleEndian(challenge.issuedAtUnixMs)
        return out
    }
}

private struct MacAuthResponse {
    let challengeID: UInt64
    let deviceID: String
    let signature: Data

    static func decode(_ payload: Data) throws -> MacAuthResponse {
        var reader = MacBinaryReader(payload)
        guard try reader.readUInt32() == 3 else {
            throw MacHostError.transportUnavailable("Payload did not contain AuthResponse")
        }
        return MacAuthResponse(
            challengeID: try reader.readUInt64(),
            deviceID: try reader.readBincodeString(),
            signature: try reader.readBincodeBytes()
        )
    }
}

private enum MacPairingResult {
    static func encode(
        accepted: Bool,
        trustedDeviceID: String?,
        reason: String?
    ) -> Data {
        var out = Data()
        out.appendLittleEndian(UInt32(5))
        out.append(accepted ? UInt8(1) : UInt8(0))
        out.appendBincodeOptionString(trustedDeviceID)
        out.appendBincodeOptionString(reason)
        return out
    }
}

private enum MacPairingChallenge {
    static func encode(salt: Data, expiresAtUnixMs: UInt64) -> Data {
        precondition(salt.count == 32)
        var out = Data()
        out.appendLittleEndian(UInt32(20))
        out.append(salt)
        out.appendLittleEndian(expiresAtUnixMs)
        out.append(UInt8(6))
        return out
    }
}

enum MacOneTimePairingCode {
    static func generate() -> String {
        let bytes = randomBytes(count: 4)
        let value = bytes.reduce(UInt32(0)) { partial, byte in
            (partial << 8) | UInt32(byte)
        } % 1_000_000
        return String(format: "%03d-%03d", value / 1_000, value % 1_000)
    }

    static func randomBytes(count: Int) -> Data {
        var generator = SystemRandomNumberGenerator()
        return Data((0..<count).map { _ in UInt8.random(in: .min ... .max, using: &generator) })
    }
}

enum MacPairingCodeProof {
    private static let domain = Data("GlyphRay pairing proof v1".utf8)

    static func create(code: String, salt: Data) -> Data? {
        let codeData = Data(code.utf8.filter { byte in byte >= 48 && byte <= 57 })
        guard codeData.count == 6, salt.count == 32 else {
            return nil
        }
        let key = SymmetricKey(data: salt)
        var message = domain
        message.append(codeData)
        return Data(HMAC<SHA256>.authenticationCode(for: message, using: key))
    }

    static func verify(code: String, salt: Data, proof: Data) -> Bool {
        let codeData = Data(code.utf8.filter { byte in byte >= 48 && byte <= 57 })
        guard codeData.count == 6, salt.count == 32 else { return false }
        let key = SymmetricKey(data: salt)
        var message = domain
        message.append(codeData)
        return HMAC<SHA256>.isValidAuthenticationCode(proof, authenticating: message, using: key)
    }
}

private enum MacDisplayInfo {
    static func encodeActiveDisplays() throws -> Data {
        #if canImport(CoreGraphics)
        var count: UInt32 = 0
        guard CGGetActiveDisplayList(0, nil, &count) == .success else {
            throw MacHostError.captureUnavailable("Unable to count active displays")
        }
        var displayIDs = [CGDirectDisplayID](repeating: 0, count: Int(count))
        guard CGGetActiveDisplayList(count, &displayIDs, &count) == .success else {
            throw MacHostError.captureUnavailable("Unable to enumerate active displays")
        }
        displayIDs = Array(displayIDs.prefix(Int(count)))

        var out = Data()
        out.appendLittleEndian(UInt32(6))
        out.appendLittleEndian(UInt64(displayIDs.count))
        for displayID in displayIDs {
            let bounds = CGDisplayBounds(displayID)
            let refreshRate = CGDisplayCopyDisplayMode(displayID)?.refreshRate ?? 0
            out.appendLittleEndian(UInt32(displayID))
            out.appendBincodeString("Display \(displayID)")
            out.appendLittleEndian(Int32(bounds.origin.x.rounded()))
            out.appendLittleEndian(Int32(bounds.origin.y.rounded()))
            out.appendLittleEndian(UInt32(CGDisplayPixelsWide(displayID)))
            out.appendLittleEndian(UInt32(CGDisplayPixelsHigh(displayID)))
            out.appendFloat32(1)
            out.appendLittleEndian(UInt16(normalizedRotation(CGDisplayRotation(displayID))))
            out.appendFloat32(Float(refreshRate > 0 ? refreshRate : 60))
            out.append(CGDisplayIsMain(displayID) != 0 ? UInt8(1) : UInt8(0))
        }
        return out
        #else
        throw MacHostError.frameworkUnavailable("CoreGraphics")
        #endif
    }

    private static func normalizedRotation(_ rotation: Double) -> Int {
        let rounded = Int(rotation.rounded()) % 360
        return rounded >= 0 ? rounded : rounded + 360
    }
}

private struct MacLatencyPing {
    let sequence: UInt64
    let clientSendTimestampUs: UInt64

    static func decode(_ payload: Data) throws -> MacLatencyPing {
        var reader = MacBinaryReader(payload)
        guard try reader.readUInt32() == 14 else {
            throw MacHostError.transportUnavailable("Payload did not contain LatencyPing")
        }
        return MacLatencyPing(
            sequence: try reader.readUInt64(),
            clientSendTimestampUs: try reader.readUInt64()
        )
    }
}

private enum MacLatencyPong {
    static func encode(
        sequence: UInt64,
        clientSendTimestampUs: UInt64,
        hostReceiveTimestampUs: UInt64,
        hostSendTimestampUs: UInt64
    ) -> Data {
        var out = Data()
        out.appendLittleEndian(UInt32(15))
        out.appendLittleEndian(sequence)
        out.appendLittleEndian(clientSendTimestampUs)
        out.appendLittleEndian(hostReceiveTimestampUs)
        out.appendLittleEndian(hostSendTimestampUs)
        return out
    }
}

private extension MacClientVideoPreference {
    static func decode(_ payload: Data) throws -> MacClientVideoPreference {
        var reader = MacBinaryReader(payload)
        guard try reader.readUInt32() == 7 else {
            throw MacHostError.transportUnavailable("Payload did not contain EncoderConfig")
        }
        return MacClientVideoPreference(
            displayID: try reader.readUInt32(),
            codec: try reader.readUInt32(),
            colorSpace: try reader.readUInt32(),
            width: try reader.readUInt32(),
            height: try reader.readUInt32(),
            maxFPS: try reader.readUInt16(),
            targetBitrateKbps: try reader.readUInt32(),
            keyframeIntervalMs: try reader.readUInt32(),
            lowLatency: try reader.readUInt8() != 0
        )
    }
}

private struct MacBinaryReader {
    private let bytes: [UInt8]
    private var offset = 0

    init(_ data: Data) {
        self.bytes = Array(data)
    }

    var isAtEnd: Bool {
        offset == bytes.count
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset + 1 <= bytes.count else {
            throw MacHostError.transportUnavailable("Unexpected end of payload")
        }
        defer { offset += 1 }
        return bytes[offset]
    }

    mutating func readUInt16() throws -> UInt16 {
        UInt16(littleEndian: try readInteger())
    }

    mutating func readUInt32() throws -> UInt32 {
        UInt32(littleEndian: try readInteger())
    }

    mutating func readUInt64() throws -> UInt64 {
        UInt64(littleEndian: try readInteger())
    }

    mutating func readBytes(count: Int) throws -> Data {
        guard count >= 0, offset + count <= bytes.count else {
            throw MacHostError.transportUnavailable("Unexpected end of payload")
        }
        defer { offset += count }
        return Data(bytes[offset..<(offset + count)])
    }

    mutating func readBincodeBytes() throws -> Data {
        let length = Int(try readUInt64())
        return try readBytes(count: length)
    }

    mutating func readBincodeString() throws -> String {
        let data = try readBincodeBytes()
        guard let string = String(data: data, encoding: .utf8) else {
            throw MacHostError.transportUnavailable("Invalid UTF-8 string")
        }
        return string
    }

    private mutating func readInteger<T: FixedWidthInteger>() throws -> T {
        let size = MemoryLayout<T>.size
        guard offset + size <= bytes.count else {
            throw MacHostError.transportUnavailable("Unexpected end of payload")
        }
        var value: T = 0
        for index in 0..<size {
            value |= T(bytes[offset + index]) << T(index * 8)
        }
        offset += size
        return value
    }
}

private extension Data {
    mutating func appendLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var littleEndian = value.littleEndian
        Swift.withUnsafeBytes(of: &littleEndian) { bytes in
            append(contentsOf: bytes)
        }
    }

    mutating func appendBincodeString(_ string: String) {
        let bytes = Data(string.utf8)
        appendLittleEndian(UInt64(bytes.count))
        append(bytes)
    }

    mutating func appendFloat32(_ value: Float) {
        appendLittleEndian(value.bitPattern)
    }

    mutating func appendBincodeOptionString(_ string: String?) {
        guard let string else {
            appendLittleEndian(UInt32(0))
            return
        }
        appendLittleEndian(UInt32(1))
        appendBincodeString(string)
    }
}

#if canImport(Network)
private extension MacUdpSendTarget {
    init?(endpoint: NWEndpoint) {
        guard case .hostPort(let host, let port) = endpoint else {
            return nil
        }
        self.init(host: "\(host)", port: port.rawValue)
    }
}
#endif

private extension MacUdpSendTarget {
    var storageKey: String {
        "\(host):\(port)"
    }
}

private func currentUnixMilliseconds() -> UInt64 {
    UInt64((Date().timeIntervalSince1970 * 1_000).rounded())
}

private func monotonicMicroseconds() -> UInt64 {
    DispatchTime.now().uptimeNanoseconds / 1_000
}

private func macControlCRC32(_ data: Data) -> UInt32 {
    var crc = UInt32.max
    for byte in data {
        let index = Int((crc ^ UInt32(byte)) & 0xff)
        crc = (crc >> 8) ^ MacControlCRC32.table[index]
    }
    return crc ^ UInt32.max
}

private enum MacControlCRC32 {
    static let table: [UInt32] = (0..<256).map { value in
        var crc = UInt32(value)
        for _ in 0..<8 {
            if crc & 1 == 1 {
                crc = 0xedb88320 ^ (crc >> 1)
            } else {
                crc >>= 1
            }
        }
        return crc
    }
}
