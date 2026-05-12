package com.glyphray.android.network

import android.view.KeyEvent

object RemoteKeyMapper {
    fun toWindowsVirtualKey(androidKeyCode: Int): Int? = when {
        androidKeyCode in KeyEvent.KEYCODE_A..KeyEvent.KEYCODE_Z ->
            0x41 + (androidKeyCode - KeyEvent.KEYCODE_A)
        androidKeyCode in KeyEvent.KEYCODE_0..KeyEvent.KEYCODE_9 ->
            0x30 + (androidKeyCode - KeyEvent.KEYCODE_0)
        androidKeyCode in KeyEvent.KEYCODE_F1..KeyEvent.KEYCODE_F12 ->
            0x70 + (androidKeyCode - KeyEvent.KEYCODE_F1)
        else -> when (androidKeyCode) {
            KeyEvent.KEYCODE_ENTER -> 0x0D
            KeyEvent.KEYCODE_DEL -> 0x08
            KeyEvent.KEYCODE_FORWARD_DEL -> 0x2E
            KeyEvent.KEYCODE_TAB -> 0x09
            KeyEvent.KEYCODE_SPACE -> 0x20
            KeyEvent.KEYCODE_ESCAPE -> 0x1B
            KeyEvent.KEYCODE_DPAD_LEFT -> 0x25
            KeyEvent.KEYCODE_DPAD_UP -> 0x26
            KeyEvent.KEYCODE_DPAD_RIGHT -> 0x27
            KeyEvent.KEYCODE_DPAD_DOWN -> 0x28
            KeyEvent.KEYCODE_MOVE_HOME -> 0x24
            KeyEvent.KEYCODE_MOVE_END -> 0x23
            KeyEvent.KEYCODE_PAGE_UP -> 0x21
            KeyEvent.KEYCODE_PAGE_DOWN -> 0x22
            KeyEvent.KEYCODE_INSERT -> 0x2D
            KeyEvent.KEYCODE_SHIFT_LEFT -> 0xA0
            KeyEvent.KEYCODE_SHIFT_RIGHT -> 0xA1
            KeyEvent.KEYCODE_CTRL_LEFT -> 0xA2
            KeyEvent.KEYCODE_CTRL_RIGHT -> 0xA3
            KeyEvent.KEYCODE_ALT_LEFT -> 0xA4
            KeyEvent.KEYCODE_ALT_RIGHT -> 0xA5
            KeyEvent.KEYCODE_META_LEFT -> 0x5B
            KeyEvent.KEYCODE_META_RIGHT -> 0x5C
            KeyEvent.KEYCODE_MENU -> 0x5D
            KeyEvent.KEYCODE_SYSRQ -> 0x2C
            KeyEvent.KEYCODE_BREAK -> 0x13
            KeyEvent.KEYCODE_CAPS_LOCK -> 0x14
            KeyEvent.KEYCODE_NUM_LOCK -> 0x90
            KeyEvent.KEYCODE_SCROLL_LOCK -> 0x91
            KeyEvent.KEYCODE_MINUS -> 0xBD
            KeyEvent.KEYCODE_EQUALS -> 0xBB
            KeyEvent.KEYCODE_LEFT_BRACKET -> 0xDB
            KeyEvent.KEYCODE_RIGHT_BRACKET -> 0xDD
            KeyEvent.KEYCODE_BACKSLASH -> 0xDC
            KeyEvent.KEYCODE_SEMICOLON -> 0xBA
            KeyEvent.KEYCODE_APOSTROPHE -> 0xDE
            KeyEvent.KEYCODE_GRAVE -> 0xC0
            KeyEvent.KEYCODE_COMMA -> 0xBC
            KeyEvent.KEYCODE_PERIOD -> 0xBE
            KeyEvent.KEYCODE_SLASH -> 0xBF
            else -> null
        }
    }
}
