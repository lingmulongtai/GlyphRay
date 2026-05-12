package com.glyphray.android.network

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class TransportPacketCodecTest {
    @Test
    fun videoFrameDatagramUsesVideoChannelAndMessageKind() {
        val fragment = byteArrayOf(1, 2, 3, 4)
        val datagram = TransportPacketCodec.encodeVideoFrame(
            sequence = 99,
            timestampUs = 1234,
            fragmentPayload = fragment,
        )

        val decoded = TransportPacketCodec.decode(datagram)

        assertEquals(TransportChannel.Video, decoded.channel)
        assertEquals(TransportMessageKind.videoFrame, decoded.messageKind)
        assertEquals(99, decoded.sequence)
        assertEquals(1234, decoded.timestampUs)
        assertArrayEquals(fragment, decoded.payload)
    }

    @Test
    fun realtimeSendQueuePrioritizesInputAndControlBeforeVideoBacklog() {
        val queue = RealtimeTransportSendQueue(capacityPerChannel = 8)
        queue.offer(TransportChannel.Video, byteArrayOf(1))
        queue.offer(TransportChannel.Video, byteArrayOf(2))
        queue.offer(TransportChannel.Control, byteArrayOf(3))
        queue.offer(TransportChannel.Input, byteArrayOf(4))

        assertEquals(TransportChannel.Input, queue.poll()?.channel)
        assertEquals(TransportChannel.Control, queue.poll()?.channel)
        assertEquals(TransportChannel.Video, queue.poll()?.channel)
    }

    @Test
    fun realtimeSendQueueDropsOldestPacketPerChannelAtCapacity() {
        val queue = RealtimeTransportSendQueue(capacityPerChannel = 2)
        queue.offer(TransportChannel.Video, byteArrayOf(1))
        queue.offer(TransportChannel.Video, byteArrayOf(2))
        queue.offer(TransportChannel.Video, byteArrayOf(3))

        assertEquals(1, queue.droppedPackets)
        assertEquals(2, queue.depth(TransportChannel.Video))
        assertEquals(2, queue.highWatermark)
    }
}
