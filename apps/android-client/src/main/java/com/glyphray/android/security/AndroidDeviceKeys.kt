package com.glyphray.android.security

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.MessageDigest
import java.security.PublicKey
import java.security.Signature
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.spec.ECGenParameterSpec

private val trustedChallengeDomain = "GlyphRay trusted device challenge v1".toByteArray(Charsets.UTF_8)

class AndroidDeviceKeys(
    private val alias: String = "glyphray_device_identity_v1",
) {
    private val keyStore: KeyStore = KeyStore.getInstance("AndroidKeyStore").also {
        it.load(null)
    }

    fun publicKey(): PublicKey {
        ensureKeyPair()
        return keyStore.getCertificate(alias).publicKey
    }

    fun publicKeyBytes(): ByteArray = publicKey().encoded

    fun trustedDeviceId(): String = "trusted-key-${publicKeyBytes().sha256Hex()}"

    fun signTrustedChallenge(
        challengeId: Long,
        nonce: ByteArray,
        trustedDeviceId: String = trustedDeviceId(),
    ): ByteArray {
        require(nonce.size == 32) { "GlyphRay auth challenge nonce must be 32 bytes" }
        val signer = Signature.getInstance("SHA256withECDSA")
        signer.initSign(ensureKeyPair().private)
        signer.update(trustedChallengePayload(trustedDeviceId, challengeId, nonce))
        return signer.sign()
    }

    private fun ensureKeyPair(): KeyPair {
        val existing = keyStore.getEntry(alias, null) as? KeyStore.PrivateKeyEntry
        if (existing != null) {
            return KeyPair(existing.certificate.publicKey, existing.privateKey)
        }

        val generator = KeyPairGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_EC,
            "AndroidKeyStore",
        )
        val spec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY,
        )
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .setUserAuthenticationRequired(false)
            .build()

        generator.initialize(spec)
        return generator.generateKeyPair()
    }
}

private fun trustedChallengePayload(
    trustedDeviceId: String,
    challengeId: Long,
    nonce: ByteArray,
): ByteArray {
    val deviceIdBytes = trustedDeviceId.toByteArray(Charsets.UTF_8)
    return ByteBuffer
        .allocate(trustedChallengeDomain.size + 8 + nonce.size + 8 + deviceIdBytes.size)
        .order(ByteOrder.LITTLE_ENDIAN)
        .put(trustedChallengeDomain)
        .putLong(challengeId)
        .put(nonce)
        .putLong(deviceIdBytes.size.toLong())
        .put(deviceIdBytes)
        .array()
}

private fun ByteArray.sha256Hex(): String {
    val digest = MessageDigest.getInstance("SHA-256").digest(this)
    return digest.joinToString(separator = "") { byte -> "%02x".format(byte) }
}
