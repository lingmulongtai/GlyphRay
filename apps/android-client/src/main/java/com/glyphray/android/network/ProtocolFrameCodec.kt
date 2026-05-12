package com.glyphray.android.network

import android.os.SystemClock
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32

private const val frameHeaderLength = 24
private const val protocolVersion: Short = 1

data class DecodedProtocolFrame(
    val sequence: Long,
    val messageKind: Int,
    val message: ControlProtocolMessage,
)

sealed interface ControlProtocolMessage {
    data class PairingResult(
        val accepted: Boolean,
        val trustedDeviceId: String?,
        val reason: String?,
    ) : ControlProtocolMessage

    data class LatencyPong(
        val sequence: Long,
        val clientSendTimestampUs: Long,
        val hostReceiveTimestampUs: Long,
        val hostSendTimestampUs: Long,
    ) : ControlProtocolMessage
}

object ProtocolFrameCodec {
    fun encodePairingRequest(
        sequence: Long,
        deviceName: String,
        pairingCodeHash: ByteArray = ByteArray(0),
        oneTimePublicKey: ByteArray = ByteArray(0),
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.pairingRequest,
        sequence = sequence,
        payload = BincodeMessageEncoder.pairingRequest(
            deviceName = deviceName,
            pairingCodeHash = pairingCodeHash,
            oneTimePublicKey = oneTimePublicKey,
        ),
    )

    fun encodeLatencyPing(
        sequence: Long,
        clientSendTimestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.latencyPing,
        sequence = sequence,
        payload = BincodeMessageEncoder.latencyPing(
            sequence = sequence,
            clientSendTimestampUs = clientSendTimestampUs,
        ),
    )

    private fun encodeFrame(
        messageKind: Int,
        sequence: Long,
        payload: ByteArray,
    ): ByteArray {
        val buffer = ByteBuffer
            .allocate(frameHeaderLength + payload.size)
            .order(ByteOrder.LITTLE_ENDIAN)
        buffer.put('G'.code.toByte())
        buffer.put('L'.code.toByte())
        buffer.put('Y'.code.toByte())
        buffer.put('R'.code.toByte())
        buffer.putShort(protocolVersion)
        buffer.putShort(messageKind.toShort())
        buffer.putLong(sequence)
        buffer.putInt(payload.size)
        buffer.putInt(payload.crc32())
        buffer.put(payload)
        return buffer.array()
    }

    fun decodeFrame(bytes: ByteArray): DecodedProtocolFrame {
        require(bytes.size >= frameHeaderLength) { "Protocol frame is too short" }
        require(bytes[0] == 'G'.code.toByte() && bytes[1] == 'L'.code.toByte() && bytes[2] == 'Y'.code.toByte() && bytes[3] == 'R'.code.toByte()) {
            "Invalid protocol frame magic"
        }

        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
        buffer.position(4)
        val version = buffer.short
        require(version == protocolVersion) { "Unsupported protocol frame version: $version" }
        val messageKind = buffer.short.toInt()
        val sequence = buffer.long
        val payloadLength = buffer.int
        require(payloadLength >= 0 && bytes.size == frameHeaderLength + payloadLength) {
            "Protocol payload length mismatch"
        }
        val expectedCrc = buffer.int
        val payload = bytes.copyOfRange(frameHeaderLength, bytes.size)
        require(payload.crc32() == expectedCrc) { "Protocol payload checksum mismatch" }

        val message = when (messageKind) {
            TransportMessageKind.pairingResult -> BincodeMessageEncoder.decodePairingResult(payload)
            TransportMessageKind.latencyPong -> BincodeMessageEncoder.decodeLatencyPong(payload)
            else -> error("Unsupported control protocol message kind: $messageKind")
        }
        return DecodedProtocolFrame(sequence = sequence, messageKind = messageKind, message = message)
    }
}

private object BincodeMessageEncoder {
    private const val pairingRequestVariant = 4
    private const val pairingResultVariant = 5
    private const val latencyPingVariant = 14
    private const val latencyPongVariant = 15

    fun pairingRequest(
        deviceName: String,
        pairingCodeHash: ByteArray,
        oneTimePublicKey: ByteArray,
    ): ByteArray {
        val nameBytes = deviceName.toByteArray(Charsets.UTF_8)
        val length = 4 + 8 + nameBytes.size + 8 + pairingCodeHash.size + 8 + oneTimePublicKey.size
        return ByteBuffer
            .allocate(length)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(pairingRequestVariant)
            .putBincodeBytes(nameBytes)
            .putBincodeBytes(pairingCodeHash)
            .putBincodeBytes(oneTimePublicKey)
            .array()
    }

    fun latencyPing(sequence: Long, clientSendTimestampUs: Long): ByteArray = ByteBuffer
        .allocate(20)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(latencyPingVariant)
        .putLong(sequence)
        .putLong(clientSendTimestampUs)
        .array()

    fun decodePairingResult(payload: ByteArray): ControlProtocolMessage.PairingResult {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == pairingResultVariant) { "Payload did not contain PairingResult" }
        val accepted = buffer.get().toInt() != 0
        return ControlProtocolMessage.PairingResult(
            accepted = accepted,
            trustedDeviceId = buffer.readBincodeOptionString(),
            reason = buffer.readBincodeOptionString(),
        )
    }

    fun decodeLatencyPong(payload: ByteArray): ControlProtocolMessage.LatencyPong {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == latencyPongVariant) { "Payload did not contain LatencyPong" }
        return ControlProtocolMessage.LatencyPong(
            sequence = buffer.long,
            clientSendTimestampUs = buffer.long,
            hostReceiveTimestampUs = buffer.long,
            hostSendTimestampUs = buffer.long,
        )
    }

    private fun ByteBuffer.putBincodeBytes(bytes: ByteArray): ByteBuffer {
        putLong(bytes.size.toLong())
        put(bytes)
        return this
    }

    private fun ByteBuffer.readBincodeOptionString(): String? {
        return when (val tag = int) {
            0 -> null
            1 -> readBincodeString()
            else -> error("Unknown bincode Option tag: $tag")
        }
    }

    private fun ByteBuffer.readBincodeString(): String {
        val length = long
        require(length >= 0 && length <= remaining()) { "Invalid bincode string length: $length" }
        val bytes = ByteArray(length.toInt())
        get(bytes)
        return bytes.toString(Charsets.UTF_8)
    }
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
