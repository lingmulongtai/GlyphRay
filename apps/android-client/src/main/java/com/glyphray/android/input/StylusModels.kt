package com.glyphray.android.input

import android.view.MotionEvent
import kotlin.math.PI

data class StylusSample(
    val pointerId: Int,
    val action: StylusAction,
    val x: Float,
    val y: Float,
    val pressure: Float,
    val tiltRadians: Float,
    val orientationRadians: Float,
    val distance: Float,
    val axisTilt: Float,
    val toolType: StylusToolType,
    val buttonState: Int,
    val eventTimeNanos: Long,
    val isHover: Boolean,
    val isEraser: Boolean,
    val isHistorical: Boolean,
) {
    val tiltDegrees: Float = tiltRadians.toDegrees()
    val orientationDegrees: Float = orientationRadians.toDegrees()
}

enum class StylusAction {
    HoverEnter,
    HoverMove,
    HoverExit,
    Down,
    Move,
    Up,
    Cancel,
    Unknown,
}

enum class StylusToolType {
    Unknown,
    Finger,
    Stylus,
    Eraser,
    Mouse,
}

data class StylusDiagnosticsState(
    val latest: StylusSample? = null,
    val totalSamples: Long = 0,
    val historicalSamples: Long = 0,
    val hoverSamples: Long = 0,
    val contactSamples: Long = 0,
    val lastBatchSize: Int = 0,
)

fun Float.toDegrees(): Float = (this * 180f / PI.toFloat())

fun MotionEvent.toStylusSamples(): List<StylusSample> {
    val samples = ArrayList<StylusSample>((historySize + 1) * pointerCount)
    val action = toStylusAction()
    val hover = action == StylusAction.HoverEnter ||
        action == StylusAction.HoverMove ||
        action == StylusAction.HoverExit

    for (historyIndex in 0 until historySize) {
        for (pointerIndex in 0 until pointerCount) {
            samples += toStylusSample(
                pointerIndex = pointerIndex,
                action = action,
                eventTimeNanos = getHistoricalEventTime(historyIndex) * 1_000_000L,
                x = getHistoricalX(pointerIndex, historyIndex),
                y = getHistoricalY(pointerIndex, historyIndex),
                pressure = getHistoricalPressure(pointerIndex, historyIndex),
                tilt = getHistoricalAxisValue(MotionEvent.AXIS_TILT, pointerIndex, historyIndex),
                orientation = getHistoricalOrientation(pointerIndex, historyIndex),
                distance = getHistoricalAxisValue(MotionEvent.AXIS_DISTANCE, pointerIndex, historyIndex),
                hover = hover,
                historical = true,
            )
        }
    }

    for (pointerIndex in 0 until pointerCount) {
        samples += toStylusSample(
            pointerIndex = pointerIndex,
            action = action,
            eventTimeNanos = eventTime * 1_000_000L,
            x = getX(pointerIndex),
            y = getY(pointerIndex),
            pressure = getPressure(pointerIndex),
            tilt = getAxisValue(MotionEvent.AXIS_TILT, pointerIndex),
            orientation = getOrientation(pointerIndex),
            distance = getAxisValue(MotionEvent.AXIS_DISTANCE, pointerIndex),
            hover = hover,
            historical = false,
        )
    }

    return samples
}

private fun MotionEvent.toStylusSample(
    pointerIndex: Int,
    action: StylusAction,
    eventTimeNanos: Long,
    x: Float,
    y: Float,
    pressure: Float,
    tilt: Float,
    orientation: Float,
    distance: Float,
    hover: Boolean,
    historical: Boolean,
): StylusSample {
    val toolType = getToolType(pointerIndex).toStylusToolType()
    return StylusSample(
        pointerId = getPointerId(pointerIndex),
        action = action,
        x = x,
        y = y,
        pressure = pressure,
        tiltRadians = tilt,
        orientationRadians = orientation,
        distance = distance,
        axisTilt = tilt,
        toolType = toolType,
        buttonState = buttonState,
        eventTimeNanos = eventTimeNanos,
        isHover = hover,
        isEraser = toolType == StylusToolType.Eraser,
        isHistorical = historical,
    )
}

private fun MotionEvent.toStylusAction(): StylusAction =
    when (actionMasked) {
        MotionEvent.ACTION_HOVER_ENTER -> StylusAction.HoverEnter
        MotionEvent.ACTION_HOVER_MOVE -> StylusAction.HoverMove
        MotionEvent.ACTION_HOVER_EXIT -> StylusAction.HoverExit
        MotionEvent.ACTION_DOWN,
        MotionEvent.ACTION_POINTER_DOWN -> StylusAction.Down
        MotionEvent.ACTION_MOVE -> StylusAction.Move
        MotionEvent.ACTION_UP,
        MotionEvent.ACTION_POINTER_UP -> StylusAction.Up
        MotionEvent.ACTION_CANCEL -> StylusAction.Cancel
        else -> StylusAction.Unknown
    }

private fun Int.toStylusToolType(): StylusToolType =
    when (this) {
        MotionEvent.TOOL_TYPE_FINGER -> StylusToolType.Finger
        MotionEvent.TOOL_TYPE_STYLUS -> StylusToolType.Stylus
        MotionEvent.TOOL_TYPE_ERASER -> StylusToolType.Eraser
        MotionEvent.TOOL_TYPE_MOUSE -> StylusToolType.Mouse
        else -> StylusToolType.Unknown
    }
