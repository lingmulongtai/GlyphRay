package com.glyphray.android.network

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.KeyFactory
import java.security.KeyPairGenerator
import java.security.MessageDigest
import java.security.PublicKey
import java.security.Signature
import java.security.spec.ECGenParameterSpec
import java.security.spec.X509EncodedKeySpec
import java.util.TreeSet
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

private val handshakeMagic = byteArrayOf('G'.code.toByte(), 'L'.code.toByte(), 'Y'.code.toByte(), 'H'.code.toByte())
private val secureMagic = byteArrayOf('G'.code.toByte(), 'L'.code.toByte(), 'Y'.code.toByte(), 'E'.code.toByte())
private const val handshakeVersion: Short = 1
private const val serverExchangeType: Byte = 1
private const val clientConfirmType: Byte = 2
private const val handshakeHeaderLength = 8
private const val secureHeaderLength = 18
private const val maxHandshakeFieldLength = 4_096
private const val maxSecureCiphertextLength = 64 * 1024
private val serverDomain = "GlyphRay server key exchange v1".toByteArray(Charsets.UTF_8)
private val clientDomain = "GlyphRay client key confirm v1".toByteArray(Charsets.UTF_8)
private val sessionKeyDomain = "GlyphRay session key v1".toByteArray(Charsets.UTF_8)
private val secureAadDomain = "GlyphRay secure datagram v1".toByteArray(Charsets.UTF_8)

data class ServerSessionKeyExchange(
    val sessionId: ByteArray,
    val expiresAtUnixMs: Long,
    val salt: ByteArray,
    val ephemeralPublicKeyDer: ByteArray,
    val hostIdentityPublicKeyDer: ByteArray,
    val signature: ByteArray,
)

data class ClientSessionKeyConfirm(
    val sessionId: ByteArray,
    val deviceId: String,
    val ephemeralPublicKeyDer: ByteArray,
    val signature: ByteArray,
)

data class SecureSessionProposal(
    val encodedClientConfirm: ByteArray,
    val codec: SecureDatagramCodec,
    val hostIdentityPublicKeyDer: ByteArray,
    val hostIdentityFingerprint: String,
)

object AndroidSessionKeyHandshake {
    fun begin(
        encodedServerExchange: ByteArray,
        deviceId: String,
        signClientPayload: (ByteArray) -> ByteArray,
        nowUnixMs: Long = System.currentTimeMillis(),
    ): SecureSessionProposal {
        val server = decodeServerExchange(encodedServerExchange)
        require(server.sessionId.size == 16 && server.salt.size == 32) {
            "Invalid secure-session identifiers"
        }
        require(nowUnixMs <= server.expiresAtUnixMs) { "Secure-session offer expired" }
        verifyServerSignature(server)

        val generator = KeyPairGenerator.getInstance("EC")
        generator.initialize(ECGenParameterSpec("secp256r1"))
        val clientEphemeral = generator.generateKeyPair()
        val unsignedConfirm = ClientSessionKeyConfirm(
            sessionId = server.sessionId.copyOf(),
            deviceId = deviceId,
            ephemeralPublicKeyDer = clientEphemeral.public.encoded,
            signature = ByteArray(0),
        )
        val serverHash = serverSigningPayload(server).sha256()
        val signature = signClientPayload(clientSigningPayload(serverHash, unsignedConfirm))
        val confirm = unsignedConfirm.copy(signature = signature)

        val agreement = KeyAgreement.getInstance("ECDH")
        agreement.init(clientEphemeral.private)
        agreement.doPhase(decodeEcPublicKey(server.ephemeralPublicKeyDer), true)
        val sharedSecret = agreement.generateSecret()
        val transcriptHash = sessionTranscriptHash(server, confirm)
        val keys = DirectionalSessionKeys.forClient(sharedSecret, transcriptHash)
        return SecureSessionProposal(
            encodedClientConfirm = encodeClientConfirm(confirm),
            codec = SecureDatagramCodec(
                outboundKey = keys.outbound,
                inboundKey = keys.inbound,
                sessionId = server.sessionId,
            ),
            hostIdentityPublicKeyDer = server.hostIdentityPublicKeyDer.copyOf(),
            hostIdentityFingerprint = server.hostIdentityPublicKeyDer.sha256Hex(),
        )
    }

    internal fun decodeServerExchange(bytes: ByteArray): ServerSessionKeyExchange {
        val buffer = handshakeBuffer(bytes, serverExchangeType)
        val sessionId = buffer.takeBytes(16)
        val expiresAt = buffer.long
        val salt = buffer.takeBytes(32)
        val ephemeral = buffer.takeLengthPrefixedBytes()
        val identity = buffer.takeLengthPrefixedBytes()
        val signature = buffer.takeLengthPrefixedBytes()
        require(!buffer.hasRemaining()) { "Secure-session exchange has trailing bytes" }
        return ServerSessionKeyExchange(sessionId, expiresAt, salt, ephemeral, identity, signature)
    }

    internal fun encodeClientConfirm(confirm: ClientSessionKeyConfirm): ByteArray {
        val deviceId = confirm.deviceId.toByteArray(Charsets.UTF_8)
        require(confirm.sessionId.size == 16) { "Session id must be 16 bytes" }
        validateHandshakeFields(deviceId, confirm.ephemeralPublicKeyDer, confirm.signature)
        return ByteBuffer.allocate(
            handshakeHeaderLength + 16 + 2 + deviceId.size +
                2 + confirm.ephemeralPublicKeyDer.size + 2 + confirm.signature.size,
        ).order(ByteOrder.LITTLE_ENDIAN)
            .put(handshakeMagic)
            .putShort(handshakeVersion)
            .put(clientConfirmType)
            .put(0)
            .put(confirm.sessionId)
            .putLengthPrefixed(deviceId)
            .putLengthPrefixed(confirm.ephemeralPublicKeyDer)
            .putLengthPrefixed(confirm.signature)
            .array()
    }

    private fun verifyServerSignature(server: ServerSessionKeyExchange) {
        val verifier = Signature.getInstance("SHA256withECDSA")
        verifier.initVerify(decodeEcPublicKey(server.hostIdentityPublicKeyDer))
        verifier.update(serverSigningPayload(server))
        require(verifier.verify(server.signature)) { "Host session-key signature did not verify" }
    }
}

data class DirectionalSessionKeys(
    val outbound: ByteArray,
    val inbound: ByteArray,
) {
    companion object {
        fun forClient(sharedSecret: ByteArray, transcriptHash: ByteArray): DirectionalSessionKeys {
            return DirectionalSessionKeys(
                outbound = deriveSessionKey(sharedSecret, transcriptHash, "client-to-host"),
                inbound = deriveSessionKey(sharedSecret, transcriptHash, "host-to-client"),
            )
        }

        fun forHost(sharedSecret: ByteArray, transcriptHash: ByteArray): DirectionalSessionKeys {
            return DirectionalSessionKeys(
                outbound = deriveSessionKey(sharedSecret, transcriptHash, "host-to-client"),
                inbound = deriveSessionKey(sharedSecret, transcriptHash, "client-to-host"),
            )
        }
    }
}

class SecureDatagramCodec(
    outboundKey: ByteArray,
    inboundKey: ByteArray,
    sessionId: ByteArray,
    private val replayWindow: Long = 4_096,
) {
    private val outboundKey = SecretKeySpec(outboundKey.copyOf(), "AES")
    private val inboundKey = SecretKeySpec(inboundKey.copyOf(), "AES")
    private val aad = secureAadDomain + sessionId.copyOf()
    private var sendCounter = 1L
    private var highestReceived: Long? = null
    private val receivedCounters = TreeSet<Long>()

    @Synchronized
    fun seal(plaintextDatagram: ByteArray): ByteArray {
        val counter = sendCounter++
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, outboundKey, GCMParameterSpec(128, nonce(counter)))
        cipher.updateAAD(aad)
        val ciphertext = cipher.doFinal(plaintextDatagram)
        require(ciphertext.size <= maxSecureCiphertextLength) { "Secure datagram is too large" }
        return ByteBuffer.allocate(secureHeaderLength + ciphertext.size)
            .order(ByteOrder.LITTLE_ENDIAN)
            .put(secureMagic)
            .putShort(handshakeVersion)
            .putLong(counter)
            .putInt(ciphertext.size)
            .put(ciphertext)
            .array()
    }

    @Synchronized
    fun open(encodedDatagram: ByteArray): ByteArray {
        require(encodedDatagram.size >= secureHeaderLength) { "Secure datagram is too short" }
        require(encodedDatagram.copyOfRange(0, 4).contentEquals(secureMagic)) {
            "Invalid secure datagram magic"
        }
        val buffer = ByteBuffer.wrap(encodedDatagram).order(ByteOrder.LITTLE_ENDIAN)
        buffer.position(4)
        require(buffer.short == handshakeVersion) { "Unsupported secure datagram version" }
        val counter = buffer.long
        val ciphertextLength = buffer.int
        require(ciphertextLength in 0..maxSecureCiphertextLength)
        require(encodedDatagram.size == secureHeaderLength + ciphertextLength) {
            "Secure datagram length mismatch"
        }
        ensureFresh(counter)

        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, inboundKey, GCMParameterSpec(128, nonce(counter)))
        cipher.updateAAD(aad)
        val plaintext = cipher.doFinal(encodedDatagram, secureHeaderLength, ciphertextLength)
        recordCounter(counter)
        return plaintext
    }

    private fun ensureFresh(counter: Long) {
        require(counter > 0) { "Secure datagram counter must be positive" }
        require(!receivedCounters.contains(counter)) { "Secure datagram replay detected" }
        val highest = highestReceived
        require(highest == null || counter + replayWindow > highest) {
            "Secure datagram counter is outside the replay window"
        }
    }

    private fun recordCounter(counter: Long) {
        highestReceived = maxOf(highestReceived ?: counter, counter)
        receivedCounters.add(counter)
        val oldest = (highestReceived ?: counter) - replayWindow + 1
        receivedCounters.headSet(oldest).clear()
    }
}

private fun serverSigningPayload(server: ServerSessionKeyExchange): ByteArray {
    return ByteBuffer.allocate(serverDomain.size + 16 + 8 + 32 + 2 + server.ephemeralPublicKeyDer.size)
        .order(ByteOrder.LITTLE_ENDIAN)
        .put(serverDomain)
        .put(server.sessionId)
        .putLong(server.expiresAtUnixMs)
        .put(server.salt)
        .putLengthPrefixed(server.ephemeralPublicKeyDer)
        .array()
}

private fun clientSigningPayload(
    serverHash: ByteArray,
    confirm: ClientSessionKeyConfirm,
): ByteArray {
    val deviceId = confirm.deviceId.toByteArray(Charsets.UTF_8)
    return ByteBuffer.allocate(
        clientDomain.size + serverHash.size + 16 + 2 + deviceId.size +
            2 + confirm.ephemeralPublicKeyDer.size,
    ).order(ByteOrder.LITTLE_ENDIAN)
        .put(clientDomain)
        .put(serverHash)
        .put(confirm.sessionId)
        .putLengthPrefixed(deviceId)
        .putLengthPrefixed(confirm.ephemeralPublicKeyDer)
        .array()
}

private fun sessionTranscriptHash(
    server: ServerSessionKeyExchange,
    confirm: ClientSessionKeyConfirm,
): ByteArray {
    val serverPayload = serverSigningPayload(server)
    val clientPayload = clientSigningPayload(serverPayload.sha256(), confirm)
    return (serverPayload + clientPayload).sha256()
}

private fun deriveSessionKey(
    sharedSecret: ByteArray,
    transcriptHash: ByteArray,
    direction: String,
): ByteArray {
    val extract = Mac.getInstance("HmacSHA256")
    extract.init(SecretKeySpec(transcriptHash, "HmacSHA256"))
    val prk = extract.doFinal(sharedSecret)
    val expand = Mac.getInstance("HmacSHA256")
    expand.init(SecretKeySpec(prk, "HmacSHA256"))
    expand.update(sessionKeyDomain)
    expand.update(direction.toByteArray(Charsets.UTF_8))
    expand.update(1)
    return expand.doFinal()
}

private fun handshakeBuffer(bytes: ByteArray, expectedType: Byte): ByteBuffer {
    require(bytes.size >= handshakeHeaderLength) { "Secure-session packet is too short" }
    require(bytes.copyOfRange(0, 4).contentEquals(handshakeMagic)) {
        "Invalid secure-session packet magic"
    }
    val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
    buffer.position(4)
    require(buffer.short == handshakeVersion) { "Unsupported secure-session version" }
    require(buffer.get() == expectedType) { "Unexpected secure-session message type" }
    buffer.get()
    return buffer
}

private fun ByteBuffer.takeLengthPrefixedBytes(): ByteArray {
    val length = short.toInt() and 0xffff
    require(length <= maxHandshakeFieldLength && remaining() >= length) {
        "Invalid secure-session field length"
    }
    return takeBytes(length)
}

private fun ByteBuffer.takeBytes(length: Int): ByteArray {
    require(length >= 0 && remaining() >= length)
    return ByteArray(length).also(::get)
}

private fun ByteBuffer.putLengthPrefixed(bytes: ByteArray): ByteBuffer {
    require(bytes.size <= maxHandshakeFieldLength)
    putShort(bytes.size.toShort())
    return put(bytes)
}

private fun validateHandshakeFields(vararg fields: ByteArray) {
    require(fields.all { it.size <= maxHandshakeFieldLength }) {
        "Secure-session handshake field is too large"
    }
}

private fun decodeEcPublicKey(der: ByteArray): PublicKey {
    return KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(der))
}

private fun nonce(counter: Long): ByteArray {
    return ByteBuffer.allocate(12)
        .order(ByteOrder.BIG_ENDIAN)
        .put(byteArrayOf('G'.code.toByte(), 'L'.code.toByte(), 'Y'.code.toByte(), 'R'.code.toByte()))
        .putLong(counter)
        .array()
}

private fun ByteArray.sha256(): ByteArray = MessageDigest.getInstance("SHA-256").digest(this)

private fun ByteArray.sha256Hex(): String = sha256().joinToString("") { "%02x".format(it) }
