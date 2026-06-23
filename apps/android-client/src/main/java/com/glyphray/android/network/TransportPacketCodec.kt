package com.glyphray.android.network

import android.os.SystemClock
import com.glyphray.android.input.StylusStreamPacket
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.zip.CRC32

private const val datagramHeaderLength = 33
private const val datagramVersion: Short = 1
private const val maxDatagramPayload = 60_000

enum class TransportChannel(val wireId: Int) {
    Video(1),
    Audio(2),
    Input(3),
    Control(4),
}

object TransportMessageKind {
    const val videoFrame = 9
    const val audioFrame = 10
    const val authChallenge = 3
    const val authResponse = 4
    const val pairingRequest = 5
    const val pairingResult = 6
    const val displayInfo = 7
    const val encoderConfig = 8
    const val stylusInputBatch = 11
    const val mouseInput = 12
    const val keyboardInput = 13
    const val latencyPing = 15
    const val latencyPong = 16
    const val touchInputBatch = 19
    const val gamepadInput = 20
    const val sessionKeyExchange = 21
    const val sessionKeyConfirm = 22
    const val pairingChallenge = 23
}

data class DecodedTransportPacket(
    val channel: TransportChannel,
    val messageKind: Int,
    val sequence: Long,
    val timestampUs: Long,
    val payload: ByteArray,
)

data class QueuedTransportDatagram(
    val channel: TransportChannel,
    val bytes: ByteArray,
)

class RealtimeTransportSendQueue(
    private val capacityPerChannel: Int = 128,
) {
    private val queues = TransportChannel.entries.associateWith { ArrayDeque<ByteArray>() }
    private var qosCursor = 0
    var droppedPackets: Long = 0
        private set
    var highWatermark: Int = 0
        private set

    fun offer(channel: TransportChannel, datagram: ByteArray) {
        val queue = queues.getValue(channel)
        if (queue.size == capacityPerChannel) {
            queue.removeFirst()
            droppedPackets += 1
        }
        queue.addLast(datagram)
        highWatermark = maxOf(highWatermark, size)
    }

    fun poll(): QueuedTransportDatagram? {
        repeat(qosSchedule.size) { offset ->
            val index = (qosCursor + offset) % qosSchedule.size
            val channel = qosSchedule[index]
            val queue = queues.getValue(channel)
            if (queue.isNotEmpty()) {
                qosCursor = (index + 1) % qosSchedule.size
                return QueuedTransportDatagram(channel, queue.removeFirst())
            }
        }
        return null
    }

    fun depth(channel: TransportChannel): Int = queues.getValue(channel).size

    val size: Int
        get() = queues.values.sumOf { it.size }

    companion object {
        private val qosSchedule = listOf(
            TransportChannel.Input,
            TransportChannel.Control,
            TransportChannel.Input,
            TransportChannel.Audio,
            TransportChannel.Control,
            TransportChannel.Video,
            TransportChannel.Input,
            TransportChannel.Control,
        )
    }
}

object TransportPacketCodec {
    fun encodeStylusInput(
        sequence: Long,
        packet: StylusStreamPacket,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encode(
        channel = TransportChannel.Input,
        messageKind = TransportMessageKind.stylusInputBatch,
        sequence = sequence,
        timestampUs = timestampUs,
        payload = packet.payload,
    )

    fun encodeControl(
        sequence: Long,
        messageKind: Int,
        payload: ByteArray,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encode(
        channel = TransportChannel.Control,
        messageKind = messageKind,
        sequence = sequence,
        timestampUs = timestampUs,
        payload = payload,
    )

    fun encodeVideoFrame(
        sequence: Long,
        fragmentPayload: ByteArray,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encode(
        channel = TransportChannel.Video,
        messageKind = TransportMessageKind.videoFrame,
        sequence = sequence,
        timestampUs = timestampUs,
        payload = fragmentPayload,
    )

    fun encodeInput(
        sequence: Long,
        messageKind: Int,
        payload: ByteArray,
        timestampUs: Long = SystemClock.elapsedRealtimeNanos() / 1_000L,
    ): ByteArray = encode(
        channel = TransportChannel.Input,
        messageKind = messageKind,
        sequence = sequence,
        timestampUs = timestampUs,
        payload = payload,
    )

    fun encode(
        channel: TransportChannel,
        messageKind: Int,
        sequence: Long,
        timestampUs: Long,
        payload: ByteArray,
    ): ByteArray {
        require(payload.size <= maxDatagramPayload) {
            "Transport payload too large: ${payload.size}"
        }

        val buffer = ByteBuffer
            .allocate(datagramHeaderLength + payload.size)
            .order(ByteOrder.LITTLE_ENDIAN)
        buffer.put('G'.code.toByte())
        buffer.put('L'.code.toByte())
        buffer.put('Y'.code.toByte())
        buffer.put('T'.code.toByte())
        buffer.putShort(datagramVersion)
        buffer.put(channel.wireId.toByte())
        buffer.putShort(messageKind.toShort())
        buffer.putLong(sequence)
        buffer.putLong(timestampUs)
        buffer.putInt(payload.size)
        buffer.putInt(payload.crc32())
        buffer.put(payload)
        return buffer.array()
    }

    fun decode(bytes: ByteArray, length: Int = bytes.size): DecodedTransportPacket {
        require(length >= datagramHeaderLength) { "Transport datagram is too short" }
        require(bytes[0] == 'G'.code.toByte() && bytes[1] == 'L'.code.toByte() && bytes[2] == 'Y'.code.toByte() && bytes[3] == 'T'.code.toByte()) {
            "Invalid transport datagram magic"
        }

        val buffer = ByteBuffer.wrap(bytes, 0, length).order(ByteOrder.LITTLE_ENDIAN)
        buffer.position(4)
        val version = buffer.short
        require(version == datagramVersion) { "Unsupported transport datagram version: $version" }
        val channel = TransportChannel.entries.firstOrNull { it.wireId == buffer.get().toInt() }
            ?: error("Unknown transport channel")
        val messageKind = buffer.short.toInt()
        val sequence = buffer.long
        val timestampUs = buffer.long
        val payloadLength = buffer.int
        require(payloadLength >= 0 && payloadLength <= maxDatagramPayload) {
            "Invalid transport payload length: $payloadLength"
        }
        require(length == datagramHeaderLength + payloadLength) {
            "Transport payload length mismatch"
        }
        val expectedCrc = buffer.int
        val payload = bytes.copyOfRange(datagramHeaderLength, length)
        require(payload.crc32() == expectedCrc) { "Transport payload checksum mismatch" }
        return DecodedTransportPacket(
            channel = channel,
            messageKind = messageKind,
            sequence = sequence,
            timestampUs = timestampUs,
            payload = payload,
        )
    }
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
