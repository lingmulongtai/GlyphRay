package com.glyphray.android.network

import android.os.SystemClock
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

private const val frameHeaderLength = 24
private const val protocolVersion: Short = 1

data class DecodedProtocolFrame(
    val sequence: Long,
    val messageKind: Int,
    val message: ControlProtocolMessage,
)

sealed interface ControlProtocolMessage {
    data class AuthChallenge(
        val challengeId: Long,
        val nonce: ByteArray,
        val issuedAtUnixMs: Long,
    ) : ControlProtocolMessage {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is AuthChallenge) return false
            return challengeId == other.challengeId &&
                nonce.contentEquals(other.nonce) &&
                issuedAtUnixMs == other.issuedAtUnixMs
        }

        override fun hashCode(): Int {
            var result = challengeId.hashCode()
            result = 31 * result + nonce.contentHashCode()
            result = 31 * result + issuedAtUnixMs.hashCode()
            return result
        }
    }

    data class PairingResult(
        val accepted: Boolean,
        val trustedDeviceId: String?,
        val reason: String?,
    ) : ControlProtocolMessage

    data class PairingChallenge(
        val salt: ByteArray,
        val expiresAtUnixMs: Long,
        val codeDigits: Int,
    ) : ControlProtocolMessage {
        override fun equals(other: Any?): Boolean =
            other is PairingChallenge &&
                salt.contentEquals(other.salt) &&
                expiresAtUnixMs == other.expiresAtUnixMs &&
                codeDigits == other.codeDigits

        override fun hashCode(): Int = 31 * (31 * salt.contentHashCode() + expiresAtUnixMs.hashCode()) + codeDigits
    }

    data class LatencyPong(
        val sequence: Long,
        val clientSendTimestampUs: Long,
        val hostReceiveTimestampUs: Long,
        val hostSendTimestampUs: Long,
    ) : ControlProtocolMessage

    data class DisplayInfo(
        val displays: List<RemoteDisplayDescriptor>,
    ) : ControlProtocolMessage

    data class AudioFrame(
        val sequence: Long,
        val captureTimestampUs: Long,
        val sampleRate: Int,
        val channels: Int,
        val payload: ByteArray,
    ) : ControlProtocolMessage {
        override fun equals(other: Any?): Boolean =
            other is AudioFrame &&
                sequence == other.sequence &&
                captureTimestampUs == other.captureTimestampUs &&
                sampleRate == other.sampleRate &&
                channels == other.channels &&
                payload.contentEquals(other.payload)

        override fun hashCode(): Int {
            var result = sequence.hashCode()
            result = 31 * result + captureTimestampUs.hashCode()
            result = 31 * result + sampleRate
            result = 31 * result + channels
            result = 31 * result + payload.contentHashCode()
            return result
        }
    }
}

data class RemoteDisplayDescriptor(
    val id: Int,
    val name: String,
    val originX: Int,
    val originY: Int,
    val widthPx: Int,
    val heightPx: Int,
    val scaleFactor: Float,
    val rotationDegrees: Int,
    val refreshHz: Float,
    val primary: Boolean,
) {
    val label: String
        get() = "$name ${widthPx}x$heightPx @ ${"%.0f".format(refreshHz)} Hz"
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

    fun encodeAuthResponse(
        sequence: Long,
        challengeId: Long,
        deviceId: String,
        signature: ByteArray,
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.authResponse,
        sequence = sequence,
        payload = BincodeMessageEncoder.authResponse(
            challengeId = challengeId,
            deviceId = deviceId,
            signature = signature,
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

    fun encodeEncoderConfig(
        sequence: Long,
        settings: ClientVideoSettings,
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.encoderConfig,
        sequence = sequence,
        payload = BincodeMessageEncoder.encoderConfig(settings),
    )

    fun encodeKeyboardInput(
        sequence: Long,
        scanCode: Int,
        virtualKey: Int,
        pressed: Boolean,
        modifiers: Int,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.keyboardInput,
        sequence = sequence,
        payload = BincodeMessageEncoder.keyboardInput(
            sequence = sequence,
            timestampUs = timestampUs,
            scanCode = scanCode,
            virtualKey = virtualKey,
            pressed = pressed,
            modifiers = modifiers,
        ),
    )

    fun encodeKeyboardInput(sequence: Long, event: KeyEvent): ByteArray? {
        val virtualKey = RemoteKeyMapper.toWindowsVirtualKey(event.keyCode) ?: return null
        return encodeKeyboardInput(
            sequence = sequence,
            scanCode = event.scanCode,
            virtualKey = virtualKey,
            pressed = event.action == KeyEvent.ACTION_DOWN,
            modifiers = event.metaState,
            timestampUs = event.eventTime * 1_000L,
        )
    }

    fun encodeMouseInput(sequence: Long, event: MotionEvent, displayId: Int = 0): ByteArray? {
        if (!event.isMouseLike()) {
            return null
        }
        return encodeMouseInput(
            sequence = sequence,
            timestampUs = event.eventTime * 1_000L,
            displayId = displayId,
            x = event.x,
            y = event.y,
            wheelDeltaX = event.getAxisValue(MotionEvent.AXIS_HSCROLL),
            wheelDeltaY = event.getAxisValue(MotionEvent.AXIS_VSCROLL),
            buttonFlags = event.buttonState,
        )
    }

    fun encodeMouseInput(
        sequence: Long,
        timestampUs: Long,
        displayId: Int = 0,
        x: Float,
        y: Float,
        wheelDeltaX: Float = 0f,
        wheelDeltaY: Float = 0f,
        buttonFlags: Int = 0,
    ): ByteArray {
        return encodeFrame(
            messageKind = TransportMessageKind.mouseInput,
            sequence = sequence,
            payload = BincodeMessageEncoder.mouseInput(
                sequence = sequence,
                timestampUs = timestampUs,
                displayId = displayId,
                x = x,
                y = y,
                wheelDeltaX = wheelDeltaX,
                wheelDeltaY = wheelDeltaY,
                buttonFlags = buttonFlags,
            ),
        )
    }

    fun encodeTouchInputBatch(sequence: Long, event: MotionEvent, displayId: Int = 0): ByteArray? {
        if (!event.isFingerTouch()) {
            return null
        }
        return encodeFrame(
            messageKind = TransportMessageKind.touchInputBatch,
            sequence = sequence,
            payload = BincodeMessageEncoder.touchInputBatch(
                batchSequence = sequence,
                monotonicTimestampUs = SystemClock.elapsedRealtimeNanos() / 1_000L,
                displayId = displayId,
                event = event,
            ),
        )
    }

    fun encodeGamepadInput(
        sequence: Long,
        controllerId: Int,
        buttons: Int,
        leftTrigger: Float,
        rightTrigger: Float,
        leftStickX: Float,
        leftStickY: Float,
        rightStickX: Float,
        rightStickY: Float,
        connected: Boolean = true,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encodeFrame(
        messageKind = TransportMessageKind.gamepadInput,
        sequence = sequence,
        payload = BincodeMessageEncoder.gamepadInput(
            sequence = sequence,
            timestampUs = timestampUs,
            controllerId = controllerId,
            connected = connected,
            buttons = buttons,
            leftTrigger = leftTrigger,
            rightTrigger = rightTrigger,
            leftStickX = leftStickX,
            leftStickY = leftStickY,
            rightStickX = rightStickX,
            rightStickY = rightStickY,
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
            TransportMessageKind.authChallenge -> BincodeMessageEncoder.decodeAuthChallenge(payload)
            TransportMessageKind.pairingResult -> BincodeMessageEncoder.decodePairingResult(payload)
            TransportMessageKind.displayInfo -> BincodeMessageEncoder.decodeDisplayInfo(payload)
            TransportMessageKind.audioFrame -> BincodeMessageEncoder.decodeAudioFrame(payload)
            TransportMessageKind.latencyPong -> BincodeMessageEncoder.decodeLatencyPong(payload)
            TransportMessageKind.pairingChallenge -> BincodeMessageEncoder.decodePairingChallenge(payload)
            else -> error("Unsupported control protocol message kind: $messageKind")
        }
        return DecodedProtocolFrame(sequence = sequence, messageKind = messageKind, message = message)
    }
}

private object BincodeMessageEncoder {
    private const val authChallengeVariant = 2
    private const val authResponseVariant = 3
    private const val pairingRequestVariant = 4
    private const val pairingResultVariant = 5
    private const val displayInfoVariant = 6
    private const val encoderConfigVariant = 7
    private const val audioFrameVariant = 9
    private const val mouseInputVariant = 11
    private const val keyboardInputVariant = 12
    private const val latencyPingVariant = 14
    private const val latencyPongVariant = 15
    private const val touchInputBatchVariant = 18
    private const val gamepadInputVariant = 19
    private const val pairingChallengeVariant = 20

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

    fun authResponse(
        challengeId: Long,
        deviceId: String,
        signature: ByteArray,
    ): ByteArray {
        val deviceIdBytes = deviceId.toByteArray(Charsets.UTF_8)
        return ByteBuffer
            .allocate(4 + 8 + 8 + deviceIdBytes.size + 8 + signature.size)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(authResponseVariant)
            .putLong(challengeId)
            .putBincodeBytes(deviceIdBytes)
            .putBincodeBytes(signature)
            .array()
    }

    fun latencyPing(sequence: Long, clientSendTimestampUs: Long): ByteArray = ByteBuffer
        .allocate(20)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(latencyPingVariant)
        .putLong(sequence)
        .putLong(clientSendTimestampUs)
        .array()

    fun encoderConfig(settings: ClientVideoSettings): ByteArray = ByteBuffer
        .allocate(4 + 4 + 4 + 4 + 4 + 4 + 2 + 4 + 4 + 1)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(encoderConfigVariant)
        .putInt(settings.displayId)
        .putInt(settings.codec.wireIndex)
        .putInt(settings.colorSpace.wireIndex)
        .putInt(settings.width)
        .putInt(settings.height)
        .putShort(settings.maxFps.toShort())
        .putInt(settings.targetBitrateKbps)
        .putInt(settings.keyframeIntervalMs)
        .put(if (settings.lowLatency) 1.toByte() else 0.toByte())
        .array()

    fun keyboardInput(
        sequence: Long,
        timestampUs: Long,
        scanCode: Int,
        virtualKey: Int,
        pressed: Boolean,
        modifiers: Int,
    ): ByteArray = ByteBuffer
        .allocate(4 + 8 + 8 + 4 + 4 + 1 + 4)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(keyboardInputVariant)
        .putLong(sequence)
        .putLong(timestampUs)
        .putInt(scanCode)
        .putInt(virtualKey)
        .put(if (pressed) 1.toByte() else 0.toByte())
        .putInt(modifiers)
        .array()

    fun mouseInput(
        sequence: Long,
        timestampUs: Long,
        displayId: Int,
        x: Float,
        y: Float,
        wheelDeltaX: Float,
        wheelDeltaY: Float,
        buttonFlags: Int,
    ): ByteArray = ByteBuffer
        .allocate(4 + 8 + 8 + 4 + 4 + 4 + 4 + 4 + 4)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(mouseInputVariant)
        .putLong(sequence)
        .putLong(timestampUs)
        .putInt(displayId)
        .putFloat(x)
        .putFloat(y)
        .putFloat(wheelDeltaX)
        .putFloat(wheelDeltaY)
        .putInt(buttonFlags)
        .array()

    fun touchInputBatch(
        batchSequence: Long,
        monotonicTimestampUs: Long,
        displayId: Int,
        event: MotionEvent,
    ): ByteArray {
        val samples = event.touchSamples()
        val sampleSize = 8 + 8 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4 + 4
        val buffer = ByteBuffer
            .allocate(4 + 8 + 8 + 4 + 8 + samples.size * sampleSize)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(touchInputBatchVariant)
            .putLong(batchSequence)
            .putLong(monotonicTimestampUs)
            .putInt(displayId)
            .putLong(samples.size.toLong())
        samples.forEach { sample ->
            buffer
                .putLong(sample.sequence)
                .putLong(sample.timestampUs)
                .putInt(sample.pointerId)
                .putInt(sample.actionWireIndex)
                .putFloat(sample.x)
                .putFloat(sample.y)
                .putFloat(sample.pressure)
                .putFloat(sample.major)
                .putFloat(sample.minor)
                .putFloat(sample.orientationDegrees)
                .putInt(sample.flags)
        }
        return buffer.array()
    }

    fun gamepadInput(
        sequence: Long,
        timestampUs: Long,
        controllerId: Int,
        connected: Boolean,
        buttons: Int,
        leftTrigger: Float,
        rightTrigger: Float,
        leftStickX: Float,
        leftStickY: Float,
        rightStickX: Float,
        rightStickY: Float,
    ): ByteArray = ByteBuffer
        .allocate(4 + 8 + 8 + 4 + 1 + 4 + 4 + 4 + 4 + 4 + 4 + 4)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(gamepadInputVariant)
        .putLong(sequence)
        .putLong(timestampUs)
        .putInt(controllerId)
        .put(if (connected) 1.toByte() else 0.toByte())
        .putInt(buttons)
        .putFloat(leftTrigger)
        .putFloat(rightTrigger)
        .putFloat(leftStickX)
        .putFloat(leftStickY)
        .putFloat(rightStickX)
        .putFloat(rightStickY)
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

    fun decodePairingChallenge(payload: ByteArray): ControlProtocolMessage.PairingChallenge {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == pairingChallengeVariant) { "Payload did not contain PairingChallenge" }
        val salt = ByteArray(32)
        buffer.get(salt)
        val expiresAtUnixMs = buffer.long
        val codeDigits = buffer.get().toInt() and 0xff
        require(codeDigits == 6) { "Unsupported pairing code length: $codeDigits" }
        require(!buffer.hasRemaining()) { "PairingChallenge contained trailing bytes" }
        return ControlProtocolMessage.PairingChallenge(salt, expiresAtUnixMs, codeDigits)
    }

    fun decodeAuthChallenge(payload: ByteArray): ControlProtocolMessage.AuthChallenge {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == authChallengeVariant) { "Payload did not contain AuthChallenge" }
        val challengeId = buffer.long
        val nonce = ByteArray(32)
        buffer.get(nonce)
        return ControlProtocolMessage.AuthChallenge(
            challengeId = challengeId,
            nonce = nonce,
            issuedAtUnixMs = buffer.long,
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

    fun decodeDisplayInfo(payload: ByteArray): ControlProtocolMessage.DisplayInfo {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == displayInfoVariant) { "Payload did not contain DisplayInfo" }
        val count = buffer.long
        require(count >= 0 && count <= 64) { "Invalid display count: $count" }
        val displays = buildList {
            repeat(count.toInt()) {
                add(buffer.readDisplayDescriptor())
            }
        }
        return ControlProtocolMessage.DisplayInfo(displays)
    }

    fun decodeAudioFrame(payload: ByteArray): ControlProtocolMessage.AudioFrame {
        val buffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
        val variant = buffer.int
        require(variant == audioFrameVariant) { "Payload did not contain AudioFrame" }
        val sequence = buffer.long
        val captureTimestampUs = buffer.long
        val sampleRate = buffer.int
        val channels = buffer.get().toInt() and 0xff
        val audioPayload = buffer.readBincodeBytes(maxLength = 256 * 1024)
        require(sampleRate in 8_000..192_000) { "Unsupported audio sample rate: $sampleRate" }
        require(channels in 1..2) { "Unsupported audio channel count: $channels" }
        require(!buffer.hasRemaining()) { "AudioFrame contained trailing bytes" }
        return ControlProtocolMessage.AudioFrame(
            sequence = sequence,
            captureTimestampUs = captureTimestampUs,
            sampleRate = sampleRate,
            channels = channels,
            payload = audioPayload,
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
        val bytes = readBincodeBytes()
        return bytes.toString(Charsets.UTF_8)
    }

    private fun ByteBuffer.readBincodeBytes(maxLength: Int = remaining()): ByteArray {
        val length = long
        require(length >= 0 && length <= remaining() && length <= maxLength) {
            "Invalid bincode byte length: $length"
        }
        val bytes = ByteArray(length.toInt())
        get(bytes)
        return bytes
    }

    private fun ByteBuffer.readDisplayDescriptor(): RemoteDisplayDescriptor {
        return RemoteDisplayDescriptor(
            id = int,
            name = readBincodeString(),
            originX = int,
            originY = int,
            widthPx = int,
            heightPx = int,
            scaleFactor = float,
            rotationDegrees = short.toInt() and 0xFFFF,
            refreshHz = float,
            primary = get().toInt() != 0,
        )
    }
}

object PairingCodeProof {
    private val domain = "GlyphRay pairing proof v1".toByteArray(Charsets.UTF_8)

    fun create(code: String, salt: ByteArray): ByteArray {
        require(salt.size == 32) { "Pairing challenge salt must be 32 bytes" }
        val canonical = code.filter(Char::isDigit)
        require(canonical.length == 6) { "Pairing code must contain six digits" }
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(salt, "HmacSHA256"))
        mac.update(domain)
        return mac.doFinal(canonical.toByteArray(Charsets.US_ASCII))
    }
}

private data class TouchWireSample(
    val sequence: Long,
    val timestampUs: Long,
    val pointerId: Int,
    val actionWireIndex: Int,
    val x: Float,
    val y: Float,
    val pressure: Float,
    val major: Float,
    val minor: Float,
    val orientationDegrees: Float,
    val flags: Int,
)

private fun MotionEvent.isMouseLike(): Boolean =
    (source and InputDevice.SOURCE_MOUSE) == InputDevice.SOURCE_MOUSE ||
        (0 until pointerCount).any { getToolType(it) == MotionEvent.TOOL_TYPE_MOUSE }

private fun MotionEvent.isFingerTouch(): Boolean =
    (0 until pointerCount).any { getToolType(it) == MotionEvent.TOOL_TYPE_FINGER }

private fun MotionEvent.touchSamples(): List<TouchWireSample> {
    val action = touchActionWireIndex()
    val samples = ArrayList<TouchWireSample>((historySize + 1) * pointerCount)
    var sequence = 1L
    for (historyIndex in 0 until historySize) {
        for (pointerIndex in 0 until pointerCount) {
            if (getToolType(pointerIndex) == MotionEvent.TOOL_TYPE_FINGER) {
                samples += touchSample(pointerIndex, action, getHistoricalEventTime(historyIndex) * 1_000L, sequence++, historyIndex)
            }
        }
    }
    for (pointerIndex in 0 until pointerCount) {
        if (getToolType(pointerIndex) == MotionEvent.TOOL_TYPE_FINGER) {
            samples += touchSample(pointerIndex, action, eventTime * 1_000L, sequence++, null)
        }
    }
    return samples
}

private fun MotionEvent.touchSample(
    pointerIndex: Int,
    actionWireIndex: Int,
    timestampUs: Long,
    sequence: Long,
    historyIndex: Int?,
): TouchWireSample {
    val xValue = historyIndex?.let { getHistoricalX(pointerIndex, it) } ?: getX(pointerIndex)
    val yValue = historyIndex?.let { getHistoricalY(pointerIndex, it) } ?: getY(pointerIndex)
    return TouchWireSample(
        sequence = sequence,
        timestampUs = timestampUs,
        pointerId = getPointerId(pointerIndex),
        actionWireIndex = actionWireIndex,
        x = xValue,
        y = yValue,
        pressure = historyIndex?.let { getHistoricalPressure(pointerIndex, it) } ?: getPressure(pointerIndex),
        major = historyIndex?.let { getHistoricalTouchMajor(pointerIndex, it) } ?: getTouchMajor(pointerIndex),
        minor = historyIndex?.let { getHistoricalTouchMinor(pointerIndex, it) } ?: getTouchMinor(pointerIndex),
        orientationDegrees = (historyIndex?.let { getHistoricalOrientation(pointerIndex, it) } ?: getOrientation(pointerIndex)) *
            180f / kotlin.math.PI.toFloat(),
        flags = 0,
    )
}

private fun MotionEvent.touchActionWireIndex(): Int =
    when (actionMasked) {
        MotionEvent.ACTION_DOWN,
        MotionEvent.ACTION_POINTER_DOWN -> 0
        MotionEvent.ACTION_UP,
        MotionEvent.ACTION_POINTER_UP -> 2
        MotionEvent.ACTION_CANCEL -> 3
        else -> 1
    }

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
