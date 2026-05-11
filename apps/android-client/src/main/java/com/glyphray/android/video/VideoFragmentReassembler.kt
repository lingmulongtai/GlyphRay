package com.glyphray.android.video

import java.nio.ByteBuffer
import java.nio.ByteOrder

private val fragmentMagic = byteArrayOf('G'.code.toByte(), 'L'.code.toByte(), 'Y'.code.toByte(), 'F'.code.toByte())
private const val fragmentHeaderLength = 20
private const val accessUnitHeaderLength = 22

enum class RemoteVideoCodec {
    H264,
    H265,
    AV1,
}

data class EncodedVideoAccessUnit(
    val sequence: Long,
    val codec: RemoteVideoCodec,
    val presentationTimeUs: Long,
    val isKeyFrame: Boolean,
    val payload: ByteArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is EncodedVideoAccessUnit) return false
        return sequence == other.sequence &&
            codec == other.codec &&
            presentationTimeUs == other.presentationTimeUs &&
            isKeyFrame == other.isKeyFrame &&
            payload.contentEquals(other.payload)
    }

    override fun hashCode(): Int {
        var result = sequence.hashCode()
        result = 31 * result + codec.hashCode()
        result = 31 * result + presentationTimeUs.hashCode()
        result = 31 * result + isKeyFrame.hashCode()
        result = 31 * result + payload.contentHashCode()
        return result
    }
}

class VideoFragmentException(message: String) : Exception(message)

class VideoFragmentReassembler {
    private val pending = mutableMapOf<Long, PendingFrame>()

    fun pushFragment(fragmentPayload: ByteArray): EncodedVideoAccessUnit? {
        val fragment = decodeFragment(fragmentPayload)
        val frame = pending.getOrPut(fragment.frameSequence) {
            PendingFrame(fragment.fragmentCount)
        }

        if (frame.fragmentCount != fragment.fragmentCount) {
            throw VideoFragmentException("Fragment count changed for frame ${fragment.frameSequence}")
        }

        if (frame.fragments[fragment.fragmentIndex] == null) {
            frame.fragments[fragment.fragmentIndex] = fragment.payload
            frame.received += 1
        }

        if (frame.received != frame.fragmentCount) {
            return null
        }

        pending.remove(fragment.frameSequence)
        return decodeAccessUnit(frame.join())
    }

    fun reset() {
        pending.clear()
    }
}

private data class VideoFragment(
    val frameSequence: Long,
    val fragmentIndex: Int,
    val fragmentCount: Int,
    val payload: ByteArray,
)

private class PendingFrame(val fragmentCount: Int) {
    var received: Int = 0
    val fragments: Array<ByteArray?> = arrayOfNulls(fragmentCount)

    fun join(): ByteArray {
        val totalSize = fragments.filterNotNull().sumOf { it.size }
        val output = ByteArray(totalSize)
        var offset = 0
        fragments.forEach { fragment ->
            requireNotNull(fragment).also {
                it.copyInto(output, destinationOffset = offset)
                offset += it.size
            }
        }
        return output
    }
}

private fun decodeFragment(bytes: ByteArray): VideoFragment {
    if (bytes.size < fragmentHeaderLength) {
        throw VideoFragmentException("Short video fragment")
    }
    if (!bytes.copyOfRange(0, 4).contentEquals(fragmentMagic)) {
        throw VideoFragmentException("Bad video fragment magic")
    }

    val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
    buffer.position(4)
    val frameSequence = buffer.long
    val fragmentIndex = buffer.short.toInt() and 0xffff
    val fragmentCount = buffer.short.toInt() and 0xffff
    val payloadLength = buffer.int

    if (fragmentCount <= 0 || fragmentIndex >= fragmentCount) {
        throw VideoFragmentException("Invalid video fragment index")
    }
    if (bytes.size != fragmentHeaderLength + payloadLength) {
        throw VideoFragmentException("Video fragment payload length mismatch")
    }

    return VideoFragment(
        frameSequence = frameSequence,
        fragmentIndex = fragmentIndex,
        fragmentCount = fragmentCount,
        payload = bytes.copyOfRange(fragmentHeaderLength, bytes.size),
    )
}

private fun decodeAccessUnit(bytes: ByteArray): EncodedVideoAccessUnit {
    if (bytes.size < accessUnitHeaderLength) {
        throw VideoFragmentException("Short encoded video access unit")
    }

    val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
    val codec = when (val codecId = buffer.get().toInt() and 0xff) {
        1 -> RemoteVideoCodec.H264
        2 -> RemoteVideoCodec.H265
        3 -> RemoteVideoCodec.AV1
        else -> throw VideoFragmentException("Unknown video codec $codecId")
    }
    val isKeyFrame = when (val flag = buffer.get().toInt() and 0xff) {
        0 -> false
        1 -> true
        else -> throw VideoFragmentException("Invalid keyframe flag $flag")
    }
    val sequence = buffer.long
    val presentationTimeUs = buffer.long
    val payloadLength = buffer.int

    if (bytes.size != accessUnitHeaderLength + payloadLength) {
        throw VideoFragmentException("Encoded video access unit length mismatch")
    }

    return EncodedVideoAccessUnit(
        sequence = sequence,
        codec = codec,
        presentationTimeUs = presentationTimeUs,
        isKeyFrame = isKeyFrame,
        payload = bytes.copyOfRange(accessUnitHeaderLength, bytes.size),
    )
}

