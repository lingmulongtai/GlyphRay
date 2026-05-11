package com.glyphray.android.security

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PublicKey
import java.security.spec.ECGenParameterSpec

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

