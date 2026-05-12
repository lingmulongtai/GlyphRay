package com.glyphray.android.network

import android.os.SystemClock
import com.glyphray.android.input.StylusStreamPacket
import java.io.Closeable
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetSocketAddress
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
    const val pairingRequest = 5
    const val stylusInputBatch = 11
    const val latencyPing = 15
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
}

class StylusUdpSender : Closeable {
    private val socket = DatagramSocket()
    private var remote: InetSocketAddress? = null
    private var nextSequence = 1L

    fun connect(host: DiscoveredHost) {
        remote = host.endpoint
        socket.connect(host.endpoint)
    }

    fun send(packet: StylusStreamPacket): Int {
        val target = remote ?: error("StylusUdpSender is not connected to a host")
        val datagram = TransportPacketCodec.encodeStylusInput(
            sequence = nextSequence++,
            packet = packet,
        )
        socket.send(DatagramPacket(datagram, datagram.size, target))
        return datagram.size
    }

    override fun close() {
        socket.close()
    }
}

private fun ByteArray.crc32(): Int {
    val crc = CRC32()
    crc.update(this)
    return crc.value.toInt()
}
