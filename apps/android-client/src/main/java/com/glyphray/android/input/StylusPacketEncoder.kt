package com.glyphray.android.input

import java.nio.ByteBuffer
import java.nio.ByteOrder

private const val stylusWireVersion: Short = 1
private const val stylusHeaderLength = 28
private const val stylusSampleLength = 58

class StylusPacketEncoder {
    private var nextBatchSequence = 1L
    private var nextSampleSequence = 1L

    fun encode(samples: List<StylusSample>, displayId: Int = 0): ByteArray {
        require(samples.size <= 65_535) {
            "Too many stylus samples in one packet: ${samples.size}"
        }

        val buffer = ByteBuffer
            .allocate(stylusHeaderLength + stylusSampleLength * samples.size)
            .order(ByteOrder.LITTLE_ENDIAN)

        buffer.put('G'.code.toByte())
        buffer.put('L'.code.toByte())
        buffer.put('Y'.code.toByte())
        buffer.put('S'.code.toByte())
        buffer.putShort(stylusWireVersion)
        buffer.putLong(nextBatchSequence++)
        buffer.putLong(samples.lastOrNull()?.eventTimeNanos?.div(1_000L) ?: 0L)
        buffer.putShort(samples.size.toShort())
        buffer.putInt(0)

        samples.forEach { sample ->
            buffer.putLong(nextSampleSequence++)
            buffer.putLong(sample.eventTimeNanos / 1_000L)
            buffer.putInt(displayId)
            buffer.putInt(sample.pointerId)
            buffer.put(sample.toolType.toWireId())
            buffer.put(sample.action.toWireId())
            buffer.putFloat(sample.x)
            buffer.putFloat(sample.y)
            buffer.putFloat(sample.pressure)
            buffer.putFloat(sample.tiltDegrees)
            buffer.putFloat(0f)
            buffer.putFloat(sample.orientationDegrees)
            buffer.putInt(sample.buttonState)
            buffer.put(sample.flagsByte())
            buffer.put(0.toByte())
            buffer.put(0.toByte())
            buffer.put(0.toByte())
        }

        return buffer.array()
    }
}

private fun StylusToolType.toWireId(): Byte =
    when (this) {
        StylusToolType.Unknown -> 0
        StylusToolType.Finger -> 1
        StylusToolType.Stylus -> 2
        StylusToolType.Eraser -> 3
        StylusToolType.Mouse -> 4
    }.toByte()

private fun StylusAction.toWireId(): Byte =
    when (this) {
        StylusAction.HoverEnter -> 0
        StylusAction.HoverMove -> 1
        StylusAction.HoverExit -> 2
        StylusAction.Down -> 3
        StylusAction.Move -> 4
        StylusAction.Up -> 5
        StylusAction.Cancel -> 6
        StylusAction.Unknown -> 6
    }.toByte()

private fun StylusSample.flagsByte(): Byte {
    var flags = 0
    if (isHover) flags = flags or 0b001
    if (isEraser) flags = flags or 0b010
    return flags.toByte()
}
