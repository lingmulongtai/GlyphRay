import CryptoKit
import Foundation

enum MacSecureSessionError: Error, CustomStringConvertible {
    case invalidPacket(String)
    case expired
    case identityMismatch
    case invalidSignature
    case replay
    case missingSession

    var description: String {
        switch self {
        case .invalidPacket(let reason): return "Invalid secure-session packet: \(reason)"
        case .expired: return "Secure-session offer expired"
        case .identityMismatch: return "Secure-session device identity did not match"
        case .invalidSignature: return "Secure-session signature did not verify"
        case .replay: return "Secure datagram replay rejected"
        case .missingSession: return "No encrypted session is established for this client"
        }
    }
}

struct MacDirectionalSessionKeys {
    let outbound: SymmetricKey
    let inbound: SymmetricKey

    static func forHost(sharedSecret: Data, transcriptHash: Data) -> MacDirectionalSessionKeys {
        MacDirectionalSessionKeys(
            outbound: derive(sharedSecret: sharedSecret, transcriptHash: transcriptHash, direction: "host-to-client"),
            inbound: derive(sharedSecret: sharedSecret, transcriptHash: transcriptHash, direction: "client-to-host")
        )
    }

    static func forClient(sharedSecret: Data, transcriptHash: Data) -> MacDirectionalSessionKeys {
        MacDirectionalSessionKeys(
            outbound: derive(sharedSecret: sharedSecret, transcriptHash: transcriptHash, direction: "client-to-host"),
            inbound: derive(sharedSecret: sharedSecret, transcriptHash: transcriptHash, direction: "host-to-client")
        )
    }

    private static func derive(
        sharedSecret: Data,
        transcriptHash: Data,
        direction: String
    ) -> SymmetricKey {
        let extractKey = SymmetricKey(data: transcriptHash)
        let pseudorandomKey = HMAC<SHA256>.authenticationCode(
            for: sharedSecret,
            using: extractKey
        )
        var info = Data("GlyphRay session key v1".utf8)
        info.append(Data(direction.utf8))
        info.append(1)
        let output = HMAC<SHA256>.authenticationCode(
            for: info,
            using: SymmetricKey(data: pseudorandomKey)
        )
        return SymmetricKey(data: output)
    }
}

final class MacHostIdentity {
    private let privateKey: P256.Signing.PrivateKey

    init(privateKey: P256.Signing.PrivateKey = P256.Signing.PrivateKey()) {
        self.privateKey = privateKey
    }

    init(rawRepresentation: Data) throws {
        privateKey = try P256.Signing.PrivateKey(rawRepresentation: rawRepresentation)
    }

    var rawRepresentation: Data { privateKey.rawRepresentation }
    var publicKeyDER: Data { privateKey.publicKey.derRepresentation }
    var fingerprint: String { MacTrustedIdentity.publicKeyFingerprint(publicKeyDER) }

    func sign(_ payload: Data) throws -> Data {
        try privateKey.signature(for: payload).derRepresentation
    }
}

final class MacHostIdentityStore {
    struct LoadResult {
        let identity: MacHostIdentity
        let quarantinedAccount: String?
    }

    private let keychain: KeychainSecretStoring
    private let account: String

    init(
        keychain: KeychainSecretStoring = KeychainSecretStore(),
        account: String = "host-signing-identity-p256-v1"
    ) {
        self.keychain = keychain
        self.account = account
    }

    func loadOrCreate() throws -> MacHostIdentity {
        try loadOrRecover().identity
    }

    func loadOrRecover() throws -> LoadResult {
        if let stored = try keychain.load(account: account) {
            do {
                return LoadResult(
                    identity: try MacHostIdentity(rawRepresentation: stored),
                    quarantinedAccount: nil
                )
            } catch {
                let quarantine = macRecoveryAccount(for: account)
                try keychain.save(stored, account: quarantine)
                try keychain.delete(account: account)
                let identity = MacHostIdentity()
                try keychain.save(identity.rawRepresentation, account: account)
                return LoadResult(identity: identity, quarantinedAccount: quarantine)
            }
        }
        let identity = MacHostIdentity()
        try keychain.save(identity.rawRepresentation, account: account)
        return LoadResult(identity: identity, quarantinedAccount: nil)
    }
}

func macRecoveryAccount(for account: String) -> String {
    let timestamp = UInt64(max(0, Date().timeIntervalSince1970 * 1_000))
    return "\(account)-corrupt-\(timestamp)-\(UUID().uuidString.lowercased())"
}

struct MacServerKeyExchange {
    let sessionID: Data
    let expiresAtUnixMs: UInt64
    let salt: Data
    let ephemeralPublicKeyDER: Data
    let hostIdentityPublicKeyDER: Data
    var signature: Data
}

struct MacClientKeyConfirm {
    let sessionID: Data
    let deviceID: String
    let ephemeralPublicKeyDER: Data
    var signature: Data
}

struct MacPendingKeyExchange {
    let exchange: MacServerKeyExchange
    let ephemeralPrivateKey: P256.KeyAgreement.PrivateKey
    let expectedDeviceID: String
    let clientIdentityPublicKeyDER: Data
}

final class MacSecureSessionCodec {
    private let outboundKey: SymmetricKey
    private let inboundKey: SymmetricKey
    private let associatedData: Data
    private let replayWindow: UInt64
    private var sendCounter: UInt64 = 1
    private var highestReceived: UInt64?
    private var receivedCounters: Set<UInt64> = []

    init(keys: MacDirectionalSessionKeys, sessionID: Data, replayWindow: UInt64 = 4_096) {
        outboundKey = keys.outbound
        inboundKey = keys.inbound
        associatedData = Data("GlyphRay secure datagram v1".utf8) + sessionID
        self.replayWindow = max(1, replayWindow)
    }

    func seal(_ plaintext: Data) throws -> Data {
        guard sendCounter > 0 else {
            throw MacSecureSessionError.invalidPacket("send counter exhausted")
        }
        let counter = sendCounter
        sendCounter &+= 1
        let nonce = try AES.GCM.Nonce(data: secureNonce(counter: counter))
        let box = try AES.GCM.seal(
            plaintext,
            using: outboundKey,
            nonce: nonce,
            authenticating: associatedData
        )
        let ciphertext = box.ciphertext + box.tag
        guard ciphertext.count <= 65_489 else {
            throw MacSecureSessionError.invalidPacket("ciphertext exceeds UDP maximum")
        }
        var output = Data("GLYE".utf8)
        output.appendSecureLittleEndian(UInt16(1))
        output.appendSecureLittleEndian(counter)
        output.appendSecureLittleEndian(UInt32(ciphertext.count))
        output.append(ciphertext)
        return output
    }

    func open(_ datagram: Data) throws -> Data {
        var reader = MacSecureBinaryReader(datagram)
        guard try reader.readData(count: 4) == Data("GLYE".utf8) else {
            throw MacSecureSessionError.invalidPacket("bad magic")
        }
        guard try reader.readUInt16() == 1 else {
            throw MacSecureSessionError.invalidPacket("unsupported version")
        }
        let counter = try reader.readUInt64()
        let ciphertextLength = Int(try reader.readUInt32())
        guard ciphertextLength >= 16, ciphertextLength <= 65_489 else {
            throw MacSecureSessionError.invalidPacket("invalid ciphertext length")
        }
        let combinedCiphertext = try reader.readData(count: ciphertextLength)
        guard reader.isAtEnd else {
            throw MacSecureSessionError.invalidPacket("trailing ciphertext bytes")
        }
        try ensureFresh(counter)

        let split = combinedCiphertext.count - 16
        let nonce = try AES.GCM.Nonce(data: secureNonce(counter: counter))
        let box = try AES.GCM.SealedBox(
            nonce: nonce,
            ciphertext: Data(combinedCiphertext.prefix(split)),
            tag: Data(combinedCiphertext.suffix(16))
        )
        let plaintext = try AES.GCM.open(box, using: inboundKey, authenticating: associatedData)
        record(counter)
        return plaintext
    }

    private func ensureFresh(_ counter: UInt64) throws {
        guard counter > 0, !receivedCounters.contains(counter) else {
            throw MacSecureSessionError.replay
        }
        if let highestReceived,
           counter <= highestReceived,
           highestReceived - counter >= replayWindow {
            throw MacSecureSessionError.replay
        }
    }

    private func record(_ counter: UInt64) {
        highestReceived = max(highestReceived ?? counter, counter)
        receivedCounters.insert(counter)
        let oldest = (highestReceived ?? counter) >= replayWindow - 1
            ? (highestReceived ?? counter) - (replayWindow - 1)
            : 0
        receivedCounters = Set(receivedCounters.filter { $0 >= oldest })
    }
}

enum MacSecureSessionHandshake {
    private static let handshakeTTLMS: UInt64 = 30_000
    private static let serverDomain = Data("GlyphRay server key exchange v1".utf8)
    private static let clientDomain = Data("GlyphRay client key confirm v1".utf8)

    static func begin(
        hostIdentity: MacHostIdentity,
        expectedDeviceID: String,
        clientIdentityPublicKeyDER: Data,
        nowUnixMs: UInt64
    ) throws -> (MacPendingKeyExchange, Data) {
        let ephemeralPrivateKey = P256.KeyAgreement.PrivateKey()
        var exchange = MacServerKeyExchange(
            sessionID: try secureRandom(count: 16),
            expiresAtUnixMs: nowUnixMs + handshakeTTLMS,
            salt: try secureRandom(count: 32),
            ephemeralPublicKeyDER: try encodeKeyAgreementPublicKey(ephemeralPrivateKey.publicKey),
            hostIdentityPublicKeyDER: hostIdentity.publicKeyDER,
            signature: Data()
        )
        exchange.signature = try hostIdentity.sign(serverSigningPayload(exchange))
        let pending = MacPendingKeyExchange(
            exchange: exchange,
            ephemeralPrivateKey: ephemeralPrivateKey,
            expectedDeviceID: expectedDeviceID,
            clientIdentityPublicKeyDER: clientIdentityPublicKeyDER
        )
        return (pending, encodeServerExchange(exchange))
    }

    static func finish(
        pending: MacPendingKeyExchange,
        encodedConfirm: Data,
        nowUnixMs: UInt64
    ) throws -> MacSecureSessionCodec {
        let confirm = try decodeClientConfirm(encodedConfirm)
        guard nowUnixMs <= pending.exchange.expiresAtUnixMs else {
            throw MacSecureSessionError.expired
        }
        guard confirm.sessionID == pending.exchange.sessionID,
              confirm.deviceID == pending.expectedDeviceID else {
            throw MacSecureSessionError.identityMismatch
        }
        let identity = try P256.Signing.PublicKey(
            derRepresentation: pending.clientIdentityPublicKeyDER
        )
        let signature = try P256.Signing.ECDSASignature(derRepresentation: confirm.signature)
        let serverHash = Data(SHA256.hash(data: serverSigningPayload(pending.exchange)))
        guard identity.isValidSignature(
            signature,
            for: clientSigningPayload(serverHash: serverHash, confirm: confirm)
        ) else {
            throw MacSecureSessionError.invalidSignature
        }
        let clientEphemeral = try decodeKeyAgreementPublicKey(confirm.ephemeralPublicKeyDER)
        let shared = try pending.ephemeralPrivateKey.sharedSecretFromKeyAgreement(
            with: clientEphemeral
        )
        let sharedData = shared.withUnsafeBytes { Data($0) }
        let transcript = sessionTranscriptHash(exchange: pending.exchange, confirm: confirm)
        return MacSecureSessionCodec(
            keys: .forHost(sharedSecret: sharedData, transcriptHash: transcript),
            sessionID: pending.exchange.sessionID
        )
    }

    static func encodeServerExchange(_ exchange: MacServerKeyExchange) -> Data {
        var output = handshakeHeader(type: 1)
        output.append(exchange.sessionID)
        output.appendSecureLittleEndian(exchange.expiresAtUnixMs)
        output.append(exchange.salt)
        output.appendSecureLengthPrefixed(exchange.ephemeralPublicKeyDER)
        output.appendSecureLengthPrefixed(exchange.hostIdentityPublicKeyDER)
        output.appendSecureLengthPrefixed(exchange.signature)
        return output
    }

    static func decodeServerExchange(_ data: Data) throws -> MacServerKeyExchange {
        var reader = try handshakeReader(data, expectedType: 1)
        let exchange = MacServerKeyExchange(
            sessionID: try reader.readData(count: 16),
            expiresAtUnixMs: try reader.readUInt64(),
            salt: try reader.readData(count: 32),
            ephemeralPublicKeyDER: try reader.readLengthPrefixed(),
            hostIdentityPublicKeyDER: try reader.readLengthPrefixed(),
            signature: try reader.readLengthPrefixed()
        )
        guard reader.isAtEnd else {
            throw MacSecureSessionError.invalidPacket("trailing server exchange bytes")
        }
        return exchange
    }

    static func encodeClientConfirm(_ confirm: MacClientKeyConfirm) -> Data {
        var output = handshakeHeader(type: 2)
        output.append(confirm.sessionID)
        output.appendSecureLengthPrefixed(Data(confirm.deviceID.utf8))
        output.appendSecureLengthPrefixed(confirm.ephemeralPublicKeyDER)
        output.appendSecureLengthPrefixed(confirm.signature)
        return output
    }

    static func decodeClientConfirm(_ data: Data) throws -> MacClientKeyConfirm {
        var reader = try handshakeReader(data, expectedType: 2)
        let sessionID = try reader.readData(count: 16)
        let deviceData = try reader.readLengthPrefixed()
        guard let deviceID = String(data: deviceData, encoding: .utf8) else {
            throw MacSecureSessionError.invalidPacket("invalid device id")
        }
        let confirm = MacClientKeyConfirm(
            sessionID: sessionID,
            deviceID: deviceID,
            ephemeralPublicKeyDER: try reader.readLengthPrefixed(),
            signature: try reader.readLengthPrefixed()
        )
        guard reader.isAtEnd else {
            throw MacSecureSessionError.invalidPacket("trailing client confirm bytes")
        }
        return confirm
    }

    static func serverSigningPayload(_ exchange: MacServerKeyExchange) -> Data {
        var output = serverDomain
        output.append(exchange.sessionID)
        output.appendSecureLittleEndian(exchange.expiresAtUnixMs)
        output.append(exchange.salt)
        output.appendSecureLengthPrefixed(exchange.ephemeralPublicKeyDER)
        return output
    }

    static func clientSigningPayload(serverHash: Data, confirm: MacClientKeyConfirm) -> Data {
        var output = clientDomain
        output.append(serverHash)
        output.append(confirm.sessionID)
        output.appendSecureLengthPrefixed(Data(confirm.deviceID.utf8))
        output.appendSecureLengthPrefixed(confirm.ephemeralPublicKeyDER)
        return output
    }

    static func sessionTranscriptHash(
        exchange: MacServerKeyExchange,
        confirm: MacClientKeyConfirm
    ) -> Data {
        let serverPayload = serverSigningPayload(exchange)
        let serverHash = Data(SHA256.hash(data: serverPayload))
        let clientPayload = clientSigningPayload(serverHash: serverHash, confirm: confirm)
        return Data(SHA256.hash(data: serverPayload + clientPayload))
    }

    static func encodeKeyAgreementPublicKey(
        _ publicKey: P256.KeyAgreement.PublicKey
    ) throws -> Data {
        try P256.Signing.PublicKey(
            x963Representation: publicKey.x963Representation
        ).derRepresentation
    }

    static func decodeKeyAgreementPublicKey(
        _ derRepresentation: Data
    ) throws -> P256.KeyAgreement.PublicKey {
        let signingKey = try P256.Signing.PublicKey(derRepresentation: derRepresentation)
        return try P256.KeyAgreement.PublicKey(
            x963Representation: signingKey.x963Representation
        )
    }

    private static func handshakeHeader(type: UInt8) -> Data {
        var output = Data("GLYH".utf8)
        output.appendSecureLittleEndian(UInt16(1))
        output.append(type)
        output.append(0)
        return output
    }

    private static func handshakeReader(
        _ data: Data,
        expectedType: UInt8
    ) throws -> MacSecureBinaryReader {
        var reader = MacSecureBinaryReader(data)
        guard try reader.readData(count: 4) == Data("GLYH".utf8),
              try reader.readUInt16() == 1,
              try reader.readUInt8() == expectedType else {
            throw MacSecureSessionError.invalidPacket("bad handshake header")
        }
        _ = try reader.readUInt8()
        return reader
    }

    private static func secureRandom(count: Int) throws -> Data {
        let bytes = try MacTrustedIdentity.makeChallengeNonce()
        return Data(bytes.prefix(count))
    }
}

private struct MacSecureBinaryReader {
    private let bytes: [UInt8]
    private(set) var offset = 0

    init(_ data: Data) { bytes = Array(data) }
    var isAtEnd: Bool { offset == bytes.count }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < bytes.count else {
            throw MacSecureSessionError.invalidPacket("unexpected end")
        }
        defer { offset += 1 }
        return bytes[offset]
    }

    mutating func readUInt16() throws -> UInt16 { try readInteger() }
    mutating func readUInt32() throws -> UInt32 { try readInteger() }
    mutating func readUInt64() throws -> UInt64 { try readInteger() }

    mutating func readData(count: Int) throws -> Data {
        guard count >= 0, offset + count <= bytes.count else {
            throw MacSecureSessionError.invalidPacket("unexpected end")
        }
        defer { offset += count }
        return Data(bytes[offset..<(offset + count)])
    }

    mutating func readLengthPrefixed() throws -> Data {
        let length = Int(try readUInt16())
        guard length <= 4_096 else {
            throw MacSecureSessionError.invalidPacket("field too large")
        }
        return try readData(count: length)
    }

    private mutating func readInteger<T: FixedWidthInteger>() throws -> T {
        let size = MemoryLayout<T>.size
        let data = try readData(count: size)
        var result: T = 0
        for (index, byte) in data.enumerated() {
            result |= T(byte) << T(index * 8)
        }
        return result
    }
}

private extension Data {
    mutating func appendSecureLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var little = value.littleEndian
        Swift.withUnsafeBytes(of: &little) { append(contentsOf: $0) }
    }

    mutating func appendSecureLengthPrefixed(_ data: Data) {
        precondition(data.count <= 4_096)
        appendSecureLittleEndian(UInt16(data.count))
        append(data)
    }
}

private func secureNonce(counter: UInt64) -> Data {
    var output = Data("GLYR".utf8)
    var big = counter.bigEndian
    Swift.withUnsafeBytes(of: &big) { output.append(contentsOf: $0) }
    return output
}
