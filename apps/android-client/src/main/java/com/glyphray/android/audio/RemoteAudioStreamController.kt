package com.glyphray.android.audio

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioTrack
import com.glyphray.android.network.ControlProtocolMessage
import com.glyphray.android.network.ProtocolFrameCodec

data class AudioFeedResult(
    val accepted: Boolean,
    val queuedBytes: Int = 0,
    val frameSequence: Long? = null,
    val reason: String? = null,
)

object RemoteAudioFrameDecoder {
    fun decode(framePayload: ByteArray): ControlProtocolMessage.AudioFrame {
        val frame = ProtocolFrameCodec.decodeFrame(framePayload)
        require(frame.message is ControlProtocolMessage.AudioFrame) {
            "Protocol frame did not contain AudioFrame"
        }
        return frame.message
    }
}

class RemoteAudioStreamController {
    private var audioTrack: AudioTrack? = null
    private var currentSampleRate: Int = 0
    private var currentChannels: Int = 0

    fun onAudioFrame(framePayload: ByteArray): AudioFeedResult {
        val frame = runCatching { RemoteAudioFrameDecoder.decode(framePayload) }
            .getOrElse { error ->
                return AudioFeedResult(accepted = false, reason = error.message)
            }

        if (frame.payload.isEmpty()) {
            return AudioFeedResult(
                accepted = false,
                frameSequence = frame.sequence,
                reason = "Audio payload was empty",
            )
        }

        val track = ensureTrack(frame.sampleRate, frame.channels)
            ?: return AudioFeedResult(
                accepted = false,
                frameSequence = frame.sequence,
                reason = "AudioTrack could not be created",
            )

        val written = track.write(frame.payload, 0, frame.payload.size, AudioTrack.WRITE_NON_BLOCKING)
        return if (written > 0) {
            AudioFeedResult(
                accepted = true,
                queuedBytes = written,
                frameSequence = frame.sequence,
            )
        } else {
            AudioFeedResult(
                accepted = false,
                frameSequence = frame.sequence,
                reason = "AudioTrack write returned $written",
            )
        }
    }

    fun release() {
        audioTrack?.release()
        audioTrack = null
        currentSampleRate = 0
        currentChannels = 0
    }

    private fun ensureTrack(sampleRate: Int, channels: Int): AudioTrack? {
        val existing = audioTrack
        if (existing != null && currentSampleRate == sampleRate && currentChannels == channels) {
            if (existing.playState != AudioTrack.PLAYSTATE_PLAYING) {
                existing.play()
            }
            return existing
        }

        release()
        val channelMask = when (channels) {
            1 -> AudioFormat.CHANNEL_OUT_MONO
            2 -> AudioFormat.CHANNEL_OUT_STEREO
            else -> return null
        }
        val minBuffer = AudioTrack.getMinBufferSize(
            sampleRate,
            channelMask,
            AudioFormat.ENCODING_PCM_16BIT,
        )
        if (minBuffer <= 0) {
            return null
        }
        val bufferBytes = maxOf(minBuffer, bytesForTwentyMilliseconds(sampleRate, channels))
        val track = runCatching {
            AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build(),
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setSampleRate(sampleRate)
                        .setChannelMask(channelMask)
                        .setEncoding(AudioFormat.ENCODING_PCM_16BIT)
                        .build(),
                )
                .setTransferMode(AudioTrack.MODE_STREAM)
                .setBufferSizeInBytes(bufferBytes)
                .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                .setSessionId(AudioManager.AUDIO_SESSION_ID_GENERATE)
                .build()
        }.getOrNull() ?: return null

        track.play()
        audioTrack = track
        currentSampleRate = sampleRate
        currentChannels = channels
        return track
    }

    private fun bytesForTwentyMilliseconds(sampleRate: Int, channels: Int): Int {
        val samples = sampleRate / 50
        return samples * channels * 2
    }
}
