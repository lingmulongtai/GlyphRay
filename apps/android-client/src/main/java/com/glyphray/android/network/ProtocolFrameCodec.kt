package com.glyphray.android.network

import android.os.SystemClock
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32

private const val frameHeaderLength = 24
private const val protocolVersion: Short = 1

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
}

private object BincodeMessageEncoder {
    private const val pairingRequestVariant = 4
    private const val latencyPingVariant = 14

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

    private fun ByteBuffer.putBincodeBytes(bytes: ByteArray): ByteBuffer {
        putLong(bytes.size.toLong())
        put(bytes)
        return this
    }
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
