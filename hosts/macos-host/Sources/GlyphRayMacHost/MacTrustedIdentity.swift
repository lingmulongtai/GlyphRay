import Foundation

#if canImport(CryptoKit)
import CryptoKit
#endif

#if canImport(Security)
import Security
#endif

enum MacTrustedIdentityError: Error, CustomStringConvertible {
    case cryptoUnavailable
    case randomUnavailable(OSStatus)
    case invalidNonceLength(Int)
    case invalidSignature

    var description: String {
        switch self {
        case .cryptoUnavailable:
            return "CryptoKit is unavailable on this platform"
        case .randomUnavailable(let status):
            return "Secure random generation failed: \(status)"
        case .invalidNonceLength(let length):
            return "Trusted auth nonce must be 32 bytes, got \(length)"
        case .invalidSignature:
            return "Trusted-device signature verification failed"
        }
    }
}

enum MacTrustedIdentity {
    static let challengeDomain = Data("GlyphRay trusted device challenge v1".utf8)

    static func publicKeyFingerprint(_ publicKeyDER: Data) -> String {
        #if canImport(CryptoKit)
        let digest = SHA256.hash(data: publicKeyDER)
        return digest.map { String(format: "%02x", $0) }.joined()
        #else
        return publicKeyDER.map { String(format: "%02x", $0) }.joined()
        #endif
    }

    static func trustedDeviceID(forPublicKeyDER publicKeyDER: Data) -> String {
        "trusted-key-\(publicKeyFingerprint(publicKeyDER))"
    }

    static func makeChallengeNonce() throws -> Data {
        #if canImport(Security)
        var data = Data(count: 32)
        let status = data.withUnsafeMutableBytes { buffer -> OSStatus in
            guard let baseAddress = buffer.baseAddress else {
                return errSecParam
            }
            return SecRandomCopyBytes(kSecRandomDefault, 32, baseAddress)
        }
        guard status == errSecSuccess else {
            throw MacTrustedIdentityError.randomUnavailable(status)
        }
        return data
        #else
        throw MacTrustedIdentityError.cryptoUnavailable
        #endif
    }

    static func makeChallengeID() throws -> UInt64 {
        let bytes = try makeChallengeNonce()
        var value: UInt64 = 0
        for (index, byte) in bytes.prefix(8).enumerated() {
            value |= UInt64(byte) << UInt64(index * 8)
        }
        return value
    }

    static func challengePayload(
        trustedDeviceID: String,
        challengeID: UInt64,
        nonce: Data
    ) throws -> Data {
        guard nonce.count == 32 else {
            throw MacTrustedIdentityError.invalidNonceLength(nonce.count)
        }
        let deviceIDBytes = Data(trustedDeviceID.utf8)
        var out = Data(capacity: challengeDomain.count + 8 + nonce.count + 8 + deviceIDBytes.count)
        out.append(challengeDomain)
        appendLittleEndian(challengeID, to: &out)
        out.append(nonce)
        appendLittleEndian(UInt64(deviceIDBytes.count), to: &out)
        out.append(deviceIDBytes)
        return out
    }

    static func verifyTrustedSignature(
        publicKeyDER: Data,
        trustedDeviceID: String,
        challengeID: UInt64,
        nonce: Data,
        signatureDER: Data
    ) throws {
        #if canImport(CryptoKit)
        let publicKey = try P256.Signing.PublicKey(derRepresentation: publicKeyDER)
        let signature = try P256.Signing.ECDSASignature(derRepresentation: signatureDER)
        let payload = try challengePayload(
            trustedDeviceID: trustedDeviceID,
            challengeID: challengeID,
            nonce: nonce
        )
        guard publicKey.isValidSignature(signature, for: payload) else {
            throw MacTrustedIdentityError.invalidSignature
        }
        #else
        throw MacTrustedIdentityError.cryptoUnavailable
        #endif
    }

    private static func appendLittleEndian<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
        var littleEndian = value.littleEndian
        Swift.withUnsafeBytes(of: &littleEndian) { bytes in
            data.append(contentsOf: bytes)
        }
    }
}
