package com.glyphray.android.video

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.view.Surface
import java.nio.ByteBuffer

data class VideoDecoderConfig(
    val width: Int,
    val height: Int,
    val mimeType: String = MediaFormat.MIMETYPE_VIDEO_AVC,
)

class VideoDecoderException(message: String, cause: Throwable? = null) : Exception(message, cause)

class RemoteVideoDecoder(private val surface: Surface) : AutoCloseable {
    private var codec: MediaCodec? = null
    private val bufferInfo = MediaCodec.BufferInfo()

    fun configure(config: VideoDecoderConfig) {
        close()

        val format = MediaFormat.createVideoFormat(config.mimeType, config.width, config.height)
        format.setInteger(MediaFormat.KEY_PRIORITY, 0)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            format.setInteger("low-latency", 1)
        }

        val decoder = try {
            MediaCodec.createDecoderByType(config.mimeType)
        } catch (error: Exception) {
            throw VideoDecoderException("No decoder available for ${config.mimeType}", error)
        }

        codec = decoder.also {
            try {
                it.configure(format, surface, null, 0)
                it.start()
            } catch (error: RuntimeException) {
                it.release()
                codec = null
                throw VideoDecoderException("Failed to configure ${config.mimeType} decoder", error)
            }
        }
    }

    fun queueAccessUnit(
        data: ByteArray,
        presentationTimeUs: Long,
        isKeyFrame: Boolean,
    ): Boolean {
        val decoder = codec ?: return false
        val inputIndex = decoder.dequeueInputBuffer(0)
        if (inputIndex < 0) {
            drainOutput(decoder)
            return false
        }

        val inputBuffer: ByteBuffer = decoder.getInputBuffer(inputIndex) ?: return false
        inputBuffer.clear()
        if (data.size > inputBuffer.capacity()) {
            decoder.queueInputBuffer(inputIndex, 0, 0, presentationTimeUs, 0)
            throw VideoDecoderException(
                "Encoded access unit is ${data.size} bytes, larger than decoder input buffer ${inputBuffer.capacity()} bytes",
            )
        }
        inputBuffer.put(data)

        val flags = if (isKeyFrame) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0
        decoder.queueInputBuffer(inputIndex, 0, data.size, presentationTimeUs, flags)
        drainOutput(decoder)
        return true
    }

    private fun drainOutput(decoder: MediaCodec) {
        while (true) {
            when (val outputIndex = decoder.dequeueOutputBuffer(bufferInfo, 0)) {
                MediaCodec.INFO_TRY_AGAIN_LATER -> return
                MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> Unit
                else -> if (outputIndex >= 0) {
                    decoder.releaseOutputBuffer(outputIndex, true)
                }
            }
        }
    }

    override fun close() {
        val decoder = codec ?: return
        codec = null
        runCatching { decoder.stop() }
        decoder.release()
    }
}
