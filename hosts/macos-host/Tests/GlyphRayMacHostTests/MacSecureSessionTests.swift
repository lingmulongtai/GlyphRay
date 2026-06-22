import CryptoKit
import XCTest
@testable import GlyphRayMacHost

final class MacSecureSessionTests: XCTestCase {
    func testDirectionalKeyDerivationMatchesRustAndKotlinVector() {
        let shared = Data((0..<32).map { UInt8(($0 * 7) & 0xff) })
        let transcript = Data((0..<32).map { UInt8(($0 * 11) & 0xff) })
        let keys = MacDirectionalSessionKeys.forClient(
            sharedSecret: shared,
            transcriptHash: transcript
        )

        XCTAssertEqual(
            keyData(keys.outbound).hex,
            "13a86c080847160ebf3331bdddd11ad8377be092698e6809c3af81fbf7c6dd0e"
        )
        XCTAssertEqual(
            keyData(keys.inbound).hex,
            "f6daad80d2a79845aa4b0f67abac4ea0412a78ff2ffcdb029874375639bc498d"
        )
    }

    func testSignedHandshakeCreatesInteroperableDirectionalCodecs() throws {
        let hostIdentity = MacHostIdentity()
        let clientIdentity = P256.Signing.PrivateKey()
        let deviceID = MacTrustedIdentity.trustedDeviceID(
            forPublicKeyDER: clientIdentity.publicKey.derRepresentation
        )
        let now: UInt64 = 1_750_000_000_000
        let (pending, encodedExchange) = try MacSecureSessionHandshake.begin(
            hostIdentity: hostIdentity,
            expectedDeviceID: deviceID,
            clientIdentityPublicKeyDER: clientIdentity.publicKey.derRepresentation,
            nowUnixMs: now
        )
        let exchange = try MacSecureSessionHandshake.decodeServerExchange(encodedExchange)

        let hostPublic = try P256.Signing.PublicKey(
            derRepresentation: exchange.hostIdentityPublicKeyDER
        )
        let hostSignature = try P256.Signing.ECDSASignature(
            derRepresentation: exchange.signature
        )
        XCTAssertTrue(hostPublic.isValidSignature(
            hostSignature,
            for: MacSecureSessionHandshake.serverSigningPayload(exchange)
        ))

        let clientEphemeral = P256.KeyAgreement.PrivateKey()
        var confirm = MacClientKeyConfirm(
            sessionID: exchange.sessionID,
            deviceID: deviceID,
            ephemeralPublicKeyDER: clientEphemeral.publicKey.derRepresentation,
            signature: Data()
        )
        let serverHash = Data(SHA256.hash(
            data: MacSecureSessionHandshake.serverSigningPayload(exchange)
        ))
        confirm.signature = try clientIdentity.signature(
            for: MacSecureSessionHandshake.clientSigningPayload(
                serverHash: serverHash,
                confirm: confirm
            )
        ).derRepresentation

        let hostEphemeral = try P256.KeyAgreement.PublicKey(
            derRepresentation: exchange.ephemeralPublicKeyDER
        )
        let shared = try clientEphemeral.sharedSecretFromKeyAgreement(with: hostEphemeral)
            .withUnsafeBytes { Data($0) }
        let transcript = MacSecureSessionHandshake.sessionTranscriptHash(
            exchange: exchange,
            confirm: confirm
        )
        let clientCodec = MacSecureSessionCodec(
            keys: .forClient(sharedSecret: shared, transcriptHash: transcript),
            sessionID: exchange.sessionID
        )
        let hostCodec = try MacSecureSessionHandshake.finish(
            pending: pending,
            encodedConfirm: MacSecureSessionHandshake.encodeClientConfirm(confirm),
            nowUnixMs: now + 1
        )

        let input = Data("stylus".utf8)
        XCTAssertEqual(try hostCodec.open(clientCodec.seal(input)), input)
        let video = Data("video".utf8)
        XCTAssertEqual(try clientCodec.open(hostCodec.seal(video)), video)
    }

    func testSecureCodecRejectsReplayButAllowsReordering() throws {
        let shared = Data(repeating: 1, count: 32)
        let transcript = Data(repeating: 2, count: 32)
        let sessionID = Data(repeating: 3, count: 16)
        let sender = MacSecureSessionCodec(
            keys: .forClient(sharedSecret: shared, transcriptHash: transcript),
            sessionID: sessionID
        )
        let receiver = MacSecureSessionCodec(
            keys: .forHost(sharedSecret: shared, transcriptHash: transcript),
            sessionID: sessionID
        )
        let first = try sender.seal(Data([1]))
        let second = try sender.seal(Data([2]))

        XCTAssertEqual(try receiver.open(second), Data([2]))
        XCTAssertEqual(try receiver.open(first), Data([1]))
        XCTAssertThrowsError(try receiver.open(first))
    }

    private func keyData(_ key: SymmetricKey) -> Data {
        key.withUnsafeBytes { Data($0) }
    }
}

private extension Data {
    var hex: String { map { String(format: "%02x", $0) }.joined() }
}
