package com.glyphray.android.network

import android.view.InputDevice
import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class RemoteGamepadMapperTest {
    @Test
    fun detectsGamepadAndJoystickSources() {
        assertTrue(RemoteGamepadMapper.isGamepadEvent(InputDevice.SOURCE_GAMEPAD))
        assertTrue(RemoteGamepadMapper.isGamepadEvent(InputDevice.SOURCE_JOYSTICK))
    }

    @Test
    fun mapsCommonControllerButtons() {
        assertEquals(RemoteGamepadMapper.BUTTON_A, RemoteGamepadMapper.buttonBit(KeyEvent.KEYCODE_BUTTON_A))
        assertEquals(RemoteGamepadMapper.BUTTON_B, RemoteGamepadMapper.buttonBit(KeyEvent.KEYCODE_BUTTON_B))
        assertEquals(RemoteGamepadMapper.DPAD_UP, RemoteGamepadMapper.buttonBit(KeyEvent.KEYCODE_DPAD_UP))
        assertNull(RemoteGamepadMapper.buttonBit(KeyEvent.KEYCODE_VOLUME_UP))
    }
}
