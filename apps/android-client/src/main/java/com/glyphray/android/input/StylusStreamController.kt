package com.glyphray.android.input

import android.view.MotionEvent

data class StylusStreamPacket(
    val payload: ByteArray,
    val sampleCount: Int,
    val latestSample: StylusSample?,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is StylusStreamPacket) return false
        return sampleCount == other.sampleCount &&
            latestSample == other.latestSample &&
            payload.contentEquals(other.payload)
    }

    override fun hashCode(): Int {
        var result = payload.contentHashCode()
        result = 31 * result + sampleCount
        result = 31 * result + (latestSample?.hashCode() ?: 0)
        return result
    }
}

class StylusStreamController(
    private val encoder: StylusPacketEncoder = StylusPacketEncoder(),
) {
    fun onMotionEvent(event: MotionEvent, displayId: Int = 0): StylusStreamPacket {
        val samples = event.toStylusSamples()
        return StylusStreamPacket(
            payload = encoder.encode(samples, displayId),
            sampleCount = samples.size,
            latestSample = samples.lastOrNull(),
        )
    }
}

