package com.glyphray.android.video

data class VideoFeedResult(
    val completedFrame: Boolean,
    val queuedToDecoder: Boolean,
    val frameSequence: Long? = null,
)

class RemoteVideoStreamController {
    private val reassembler = VideoFragmentReassembler()
    private var decoder: RemoteVideoDecoder? = null

    fun attachDecoder(decoder: RemoteVideoDecoder) {
        this.decoder = decoder
        reassembler.reset()
    }

    fun detachDecoder() {
        decoder = null
        reassembler.reset()
    }

    fun onVideoFragment(fragmentPayload: ByteArray): VideoFeedResult {
        val accessUnit = reassembler.pushFragment(fragmentPayload)
            ?: return VideoFeedResult(completedFrame = false, queuedToDecoder = false)

        if (accessUnit.codec != RemoteVideoCodec.H264) {
            return VideoFeedResult(
                completedFrame = true,
                queuedToDecoder = false,
                frameSequence = accessUnit.sequence,
            )
        }

        val queued = decoder?.queueAccessUnit(
            data = accessUnit.payload,
            presentationTimeUs = accessUnit.presentationTimeUs,
            isKeyFrame = accessUnit.isKeyFrame,
        ) ?: false

        return VideoFeedResult(
            completedFrame = true,
            queuedToDecoder = queued,
            frameSequence = accessUnit.sequence,
        )
    }
}

