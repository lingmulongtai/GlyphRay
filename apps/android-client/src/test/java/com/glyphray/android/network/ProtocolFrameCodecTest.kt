package com.glyphray.android.network

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
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

    @Test
    fun pairingResultFrameDecodesFromRustBincodeLayout() {
        val payload = ByteBuffer
            .allocate(4 + 1 + 4 + 8 + "trusted-device".length + 4)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(5)
            .put(1.toByte())
            .putInt(1)
            .putLong("trusted-device".length.toLong())
            .put("trusted-device".toByteArray(Charsets.UTF_8))
            .putInt(0)
            .array()

        val frame = ProtocolFrameCodec.decodeFrame(encodeFrame(42, TransportMessageKind.pairingResult, payload))
        val message = frame.message as ControlProtocolMessage.PairingResult

        assertEquals(42, frame.sequence)
        assertEquals(true, message.accepted)
        assertEquals("trusted-device", message.trustedDeviceId)
        assertNull(message.reason)
    }

    @Test
    fun authChallengeFrameDecodesFromRustBincodeLayout() {
        val nonce = ByteArray(32) { index -> index.toByte() }
        val payload = ByteBuffer
            .allocate(4 + 8 + 32 + 8)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(2)
            .putLong(99)
            .put(nonce)
            .putLong(1_800_000)
            .array()

        val frame = ProtocolFrameCodec.decodeFrame(encodeFrame(45, TransportMessageKind.authChallenge, payload))
        val message = frame.message as ControlProtocolMessage.AuthChallenge

        assertEquals(45, frame.sequence)
        assertEquals(99, message.challengeId)
        assertArrayEquals(nonce, message.nonce)
        assertEquals(1_800_000, message.issuedAtUnixMs)
    }

    @Test
    fun authResponseFrameUsesRustBincodeLayout() {
        val frame = ProtocolFrameCodec.encodeAuthResponse(
            sequence = 46,
            challengeId = 99,
            deviceId = "trusted-key-test",
            signature = byteArrayOf(7, 8, 9),
        )
        val payload = frame.copyOfRange(24, frame.size)
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)

        assertEquals(3, buffer.int)
        assertEquals(99, buffer.long)

        val deviceIdLength = buffer.long.toInt()
        val deviceId = ByteArray(deviceIdLength)
        buffer.get(deviceId)
        assertEquals("trusted-key-test", deviceId.toString(Charsets.UTF_8))

        val signatureLength = buffer.long.toInt()
        val signature = ByteArray(signatureLength)
        buffer.get(signature)
        assertArrayEquals(byteArrayOf(7, 8, 9), signature)
    }

    @Test
    fun latencyPongFrameDecodesFromRustBincodeLayout() {
        val payload = ByteBuffer
            .allocate(36)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(15)
            .putLong(3)
            .putLong(100)
            .putLong(130)
            .putLong(140)
            .array()

        val frame = ProtocolFrameCodec.decodeFrame(encodeFrame(43, TransportMessageKind.latencyPong, payload))
        val message = frame.message as ControlProtocolMessage.LatencyPong

        assertEquals(43, frame.sequence)
        assertEquals(3, message.sequence)
        assertEquals(100, message.clientSendTimestampUs)
        assertEquals(130, message.hostReceiveTimestampUs)
        assertEquals(140, message.hostSendTimestampUs)
    }

    @Test
    fun displayInfoFrameDecodesRustDisplayDescriptors() {
        val name = "\\\\.\\DISPLAY1".toByteArray(Charsets.UTF_8)
        val payload = ByteBuffer
            .allocate(4 + 8 + 4 + 8 + name.size + 4 + 4 + 4 + 4 + 4 + 2 + 4 + 1)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(6)
            .putLong(1)
            .putInt(7)
            .putLong(name.size.toLong())
            .put(name)
            .putInt(0)
            .putInt(0)
            .putInt(1920)
            .putInt(1080)
            .putFloat(1.25f)
            .putShort(0.toShort())
            .putFloat(60.0f)
            .put(1.toByte())
            .array()

        val frame = ProtocolFrameCodec.decodeFrame(encodeFrame(44, TransportMessageKind.displayInfo, payload))
        val message = frame.message as ControlProtocolMessage.DisplayInfo

        assertEquals(1, message.displays.size)
        assertEquals(7, message.displays[0].id)
        assertEquals("\\\\.\\DISPLAY1", message.displays[0].name)
        assertEquals(1920, message.displays[0].widthPx)
        assertEquals(1080, message.displays[0].heightPx)
        assertEquals(true, message.displays[0].primary)
    }

    @Test
    fun encoderConfigFrameUsesRustBincodeLayout() {
        val frame = ProtocolFrameCodec.encodeEncoderConfig(
            sequence = 55,
            settings = ClientVideoSettings(
                displayId = 2,
                resolution = ClientResolution.R1440p,
                codec = ClientVideoCodec.H265,
                colorSpace = ClientColorSpace.DisplayP3,
                maxFps = 120,
                targetBitrateKbps = 35_000,
                keyframeIntervalMs = 500,
                lowLatency = true,
            ),
        )
        val payload = frame.copyOfRange(24, frame.size)
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)

        assertEquals(7, buffer.int)
        assertEquals(2, buffer.int)
        assertEquals(ClientVideoCodec.H265.wireIndex, buffer.int)
        assertEquals(ClientColorSpace.DisplayP3.wireIndex, buffer.int)
        assertEquals(2560, buffer.int)
        assertEquals(1440, buffer.int)
        assertEquals(120, buffer.short.toInt())
        assertEquals(35_000, buffer.int)
        assertEquals(500, buffer.int)
        assertEquals(1, buffer.get().toInt())
    }

    @Test
    fun keyboardInputFrameUsesRustBincodeLayout() {
        val frame = ProtocolFrameCodec.encodeKeyboardInput(
            sequence = 56,
            scanCode = 0x37,
            virtualKey = 0x2C,
            pressed = true,
            modifiers = 0,
            timestampUs = 1234,
        )
        val payload = frame.copyOfRange(24, frame.size)
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)

        assertEquals(12, buffer.int)
        assertEquals(56, buffer.long)
        assertEquals(1234, buffer.long)
        assertEquals(0x37, buffer.int)
        assertEquals(0x2C, buffer.int)
        assertEquals(1, buffer.get().toInt())
        assertEquals(0, buffer.int)
    }

    @Test
    fun syntheticMouseInputFrameUsesRustBincodeLayout() {
        val frame = ProtocolFrameCodec.encodeMouseInput(
            sequence = 57,
            timestampUs = 12345,
            displayId = 2,
            x = 321.5f,
            y = 654.25f,
            wheelDeltaX = 1.0f,
            wheelDeltaY = -2.0f,
            buttonFlags = 1,
        )
        val payload = frame.copyOfRange(24, frame.size)
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)

        assertEquals(11, buffer.int)
        assertEquals(57, buffer.long)
        assertEquals(12345, buffer.long)
        assertEquals(2, buffer.int)
        assertEquals(321.5f, buffer.float)
        assertEquals(654.25f, buffer.float)
        assertEquals(1.0f, buffer.float)
        assertEquals(-2.0f, buffer.float)
        assertEquals(1, buffer.int)
    }
}

private fun encodeFrame(sequence: Long, messageKind: Int, payload: ByteArray): ByteArray {
    return ByteBuffer
        .allocate(24 + payload.size)
        .order(ByteOrder.LITTLE_ENDIAN)
        .put('G'.code.toByte())
        .put('L'.code.toByte())
        .put('Y'.code.toByte())
        .put('R'.code.toByte())
        .putShort(1.toShort())
        .putShort(messageKind.toShort())
        .putLong(sequence)
        .putInt(payload.size)
        .putInt(payload.crc32())
        .put(payload)
        .array()
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
