package com.glyphray.android.network

import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import kotlin.math.abs

object RemoteGamepadMapper {
    const val BUTTON_A = 1 shl 0
    const val BUTTON_B = 1 shl 1
    const val BUTTON_X = 1 shl 2
    const val BUTTON_Y = 1 shl 3
    const val BUTTON_L1 = 1 shl 4
    const val BUTTON_R1 = 1 shl 5
    const val BUTTON_BACK = 1 shl 6
    const val BUTTON_START = 1 shl 7
    const val BUTTON_LEFT_STICK = 1 shl 8
    const val BUTTON_RIGHT_STICK = 1 shl 9
    const val DPAD_UP = 1 shl 10
    const val DPAD_DOWN = 1 shl 11
    const val DPAD_LEFT = 1 shl 12
    const val DPAD_RIGHT = 1 shl 13

    fun isGamepadEvent(source: Int): Boolean =
        (source and InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD ||
            (source and InputDevice.SOURCE_JOYSTICK) == InputDevice.SOURCE_JOYSTICK

    fun buttonBit(keyCode: Int): Int? = when (keyCode) {
        KeyEvent.KEYCODE_BUTTON_A -> BUTTON_A
        KeyEvent.KEYCODE_BUTTON_B -> BUTTON_B
        KeyEvent.KEYCODE_BUTTON_X -> BUTTON_X
        KeyEvent.KEYCODE_BUTTON_Y -> BUTTON_Y
        KeyEvent.KEYCODE_BUTTON_L1 -> BUTTON_L1
        KeyEvent.KEYCODE_BUTTON_R1 -> BUTTON_R1
        KeyEvent.KEYCODE_BUTTON_SELECT -> BUTTON_BACK
        KeyEvent.KEYCODE_BUTTON_START -> BUTTON_START
        KeyEvent.KEYCODE_BUTTON_THUMBL -> BUTTON_LEFT_STICK
        KeyEvent.KEYCODE_BUTTON_THUMBR -> BUTTON_RIGHT_STICK
        KeyEvent.KEYCODE_DPAD_UP -> DPAD_UP
        KeyEvent.KEYCODE_DPAD_DOWN -> DPAD_DOWN
        KeyEvent.KEYCODE_DPAD_LEFT -> DPAD_LEFT
        KeyEvent.KEYCODE_DPAD_RIGHT -> DPAD_RIGHT
        else -> null
    }

    fun axis(event: MotionEvent, primary: Int, fallback: Int? = null): Float {
        val primaryValue = event.getAxisValue(primary).deadzone()
        if (primaryValue != 0f || fallback == null) {
            return primaryValue
        }
        return event.getAxisValue(fallback).deadzone()
    }

    private fun Float.deadzone(): Float = if (abs(this) < 0.08f) 0f else this.coerceIn(-1f, 1f)
}
