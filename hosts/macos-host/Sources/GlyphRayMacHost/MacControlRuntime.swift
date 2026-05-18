import Foundation

#if canImport(Network)
import Network
#endif

private let macControlDefaultPort: UInt16 = 44_999

struct MacPairingClient: Identifiable, Equatable {
    let id: String
    let deviceName: String
    let target: MacUdpSendTarget
    let publicKeyFingerprint: String?
    let pairedAtUnixMs: UInt64
}

struct MacClientVideoPreference: Equatable {
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
    let lastVideoPreference: MacClientVideoPreference?
    let lastEvent: String
}

final class MacControlRuntime {
    var onSnapshot: ((MacControlRuntimeSnapshot) -> Void)?

    private let queue = DispatchQueue(label: "com.glyphray.mac.control-runtime")
    private var bindPort: UInt16 = macControlDefaultPort
    private var pairingRequestsReceived: UInt64 = 0
    private var acceptedClients: [MacPairingClient] = []
    private var lastApprovedTarget: MacUdpSendTarget?
    private var lastVideoPreference: MacClientVideoPreference?
    private var lastEvent = "Control runtime idle"

    #if canImport(Network)
    private var listener: NWListener?
    private var connections: [NWConnection] = []
    #endif

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
            if case .failed(let error) = state {
                self?.queue.async {
                    self?.lastEvent = "Control peer failed: \(error)"
                    self?.publishSnapshot()
                }
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

    private func handleDatagram(_ data: Data, from connection: NWConnection) {
        guard let packet = try? MacTransportDatagram.decode(data), packet.channel == .control else {
            return
        }
        guard let frame = try? MacProtocolFrame.decode(packet.payload) else {
            lastEvent = "Ignored malformed control frame"
            publishSnapshot()
            return
        }

        switch frame.messageKind {
        case .pairingRequest:
            handlePairingRequest(frame: frame, connection: connection)
        case .encoderConfig:
            if let preference = try? MacClientVideoPreference.decode(frame.payload) {
                lastVideoPreference = preference
                lastEvent = "Client requested \(preference.width)x\(preference.height) @ \(preference.maxFPS)fps"
                publishSnapshot()
            }
        case .latencyPing:
            handleLatencyPing(frame: frame, connection: connection)
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
        let fingerprint = request.oneTimePublicKey.isEmpty ? nil : shortFingerprint(for: request.oneTimePublicKey)
        let trustedID = fingerprint.map { "mac-\($0)" } ?? "mac-\(target.host)-\(target.port)"
        let client = MacPairingClient(
            id: trustedID,
            deviceName: request.deviceName,
            target: target,
            publicKeyFingerprint: fingerprint,
            pairedAtUnixMs: currentUnixMilliseconds()
        )

        acceptedClients.removeAll { $0.id == client.id || $0.target == client.target }
        acceptedClients.insert(client, at: 0)
        if acceptedClients.count > 8 {
            acceptedClients.removeLast(acceptedClients.count - 8)
        }
        lastApprovedTarget = target
        lastEvent = "Accepted \(request.deviceName) at \(target.host):\(target.port)"

        let responseFrame = MacProtocolFrame.encode(
            sequence: frame.sequence,
            messageKind: .pairingResult,
            payload: MacPairingResult.accepted(trustedDeviceID: trustedID)
        )
        let responseDatagram = MacTransportDatagram.encode(
            channel: .control,
            messageKind: .pairingResult,
            sequence: frame.sequence,
            timestampUs: monotonicMicroseconds(),
            payload: responseFrame
        )
        connection.send(content: responseDatagram, completion: .contentProcessed { _ in })
        publishSnapshot()
    }

    private func handleLatencyPing(
        frame: MacProtocolFrame,
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
        connection.send(content: datagram, completion: .contentProcessed { _ in })
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
            lastVideoPreference: lastVideoPreference,
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
    case latencyPing = 15
    case latencyPong = 16
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
    let oneTimePublicKey: Data

    static func decode(_ payload: Data) throws -> MacPairingRequest {
        var reader = MacBinaryReader(payload)
        guard try reader.readUInt32() == 4 else {
            throw MacHostError.transportUnavailable("Payload did not contain PairingRequest")
        }
        let deviceName = try reader.readBincodeString()
        _ = try reader.readBincodeBytes()
        let oneTimePublicKey = try reader.readBincodeBytes()
        return MacPairingRequest(deviceName: deviceName, oneTimePublicKey: oneTimePublicKey)
    }
}

private enum MacPairingResult {
    static func accepted(trustedDeviceID: String) -> Data {
        var out = Data()
        out.appendLittleEndian(UInt32(5))
        out.append(UInt8(1))
        out.appendLittleEndian(UInt32(1))
        out.appendBincodeString(trustedDeviceID)
        out.appendLittleEndian(UInt32(0))
        return out
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

private func currentUnixMilliseconds() -> UInt64 {
    UInt64((Date().timeIntervalSince1970 * 1_000).rounded())
}

private func monotonicMicroseconds() -> UInt64 {
    DispatchTime.now().uptimeNanoseconds / 1_000
}

private func shortFingerprint(for data: Data) -> String {
    let crc = macControlCRC32(data)
    return String(format: "%08x", crc)
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
