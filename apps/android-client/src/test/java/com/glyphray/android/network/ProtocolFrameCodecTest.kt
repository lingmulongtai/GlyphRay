package com.glyphray.android.network

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32

class ProtocolFrameCodecTest {
    @Test
    fun latencyPingFrameMatchesRustProtocolEnvelope() {
        val frame = ProtocolFrameCodec.encodeLatencyPing(
            sequence = 7,
            clientSendTimestampUs = 123_456,
        )
        val buffer = ByteBuffer.wrap(frame).order(ByteOrder.LITTLE_ENDIAN)

        assertArrayEquals(byteArrayOf('G'.code.toByte(), 'L'.code.toByte(), 'Y'.code.toByte(), 'R'.code.toByte()), frame.copyOfRange(0, 4))
        buffer.position(4)
        assertEquals(1, buffer.short.toInt())
        assertEquals(TransportMessageKind.latencyPing, buffer.short.toInt())
        assertEquals(7, buffer.long)
        assertEquals(20, buffer.int)

        val expectedCrc = buffer.int
        val payload = frame.copyOfRange(24, frame.size)
        assertEquals(expectedCrc, payload.crc32())

        val payloadBuffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(14, payloadBuffer.int)
        assertEquals(7, payloadBuffer.long)
        assertEquals(123_456, payloadBuffer.long)
    }

    @Test
    fun pairingRequestUsesBincodeStringAndByteVectorLayout() {
        val frame = ProtocolFrameCodec.encodePairingRequest(
            sequence = 9,
            deviceName = "GlyphRay Android Test",
            pairingCodeHash = byteArrayOf(1, 2, 3),
            oneTimePublicKey = byteArrayOf(4, 5),
        )
        val payload = frame.copyOfRange(24, frame.size)
        val payloadBuffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)

        assertEquals(4, payloadBuffer.int)

        val nameLength = payloadBuffer.long.toInt()
        val nameBytes = ByteArray(nameLength)
        payloadBuffer.get(nameBytes)
        assertEquals("GlyphRay Android Test", nameBytes.toString(Charsets.UTF_8))

        val hashLength = payloadBuffer.long.toInt()
        val hash = ByteArray(hashLength)
        payloadBuffer.get(hash)
        assertArrayEquals(byteArrayOf(1, 2, 3), hash)

        val keyLength = payloadBuffer.long.toInt()
        val key = ByteArray(keyLength)
        payloadBuffer.get(key)
        assertArrayEquals(byteArrayOf(4, 5), key)
    }
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
