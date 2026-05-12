package com.glyphray.android.network

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class RemoteKeyMapperTest {
    @Test
    fun mapsLettersDigitsAndSpecialKeysToWindowsVirtualKeys() {
        assertEquals(0x41, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_A))
        assertEquals(0x5A, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_Z))
        assertEquals(0x30, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_0))
        assertEquals(0x39, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_9))
        assertEquals(0x70, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_F1))
        assertEquals(0x7B, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_F12))
        assertEquals(0x5B, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_META_LEFT))
        assertEquals(0x2C, RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_SYSRQ))
    }

    @Test
    fun leavesUnsupportedAndroidOnlyKeysUnhandled() {
        assertNull(RemoteKeyMapper.toWindowsVirtualKey(KeyEvent.KEYCODE_VOLUME_UP))
    }
}
