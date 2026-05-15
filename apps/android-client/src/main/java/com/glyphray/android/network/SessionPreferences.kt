package com.glyphray.android.network

import android.content.Context

enum class ClientVideoCodec(val wireIndex: Int, val label: String) {
    H264(0, "H.264"),
    H265(1, "H.265"),
    AV1(2, "AV1"),
}

enum class ClientColorSpace(val wireIndex: Int, val label: String) {
    Srgb(0, "sRGB"),
    DisplayP3(1, "Display P3"),
    Rec709(2, "Rec.709"),
    Rec2020Pq(3, "Rec.2020 PQ"),
}

enum class ClientResolution(val width: Int, val height: Int, val label: String) {
    R1080p(1920, 1080, "1920 x 1080"),
    R1440p(2560, 1440, "2560 x 1440"),
    Native(0, 0, "Host native"),
}

enum class ClientTouchMode(val label: String) {
    Direct("Direct touch"),
    Trackpad("Trackpad"),
    Gesture("Gesture assist"),
}

enum class SpecialRemoteKey(val label: String, val virtualKey: Int, val scanCode: Int) {
    Windows("Win", 0x5B, 0),
    PrintScreen("PrtSc", 0x2C, 0x37),
}

data class ClientVideoSettings(
    val displayId: Int = 0,
    val resolution: ClientResolution = ClientResolution.R1080p,
    val codec: ClientVideoCodec = ClientVideoCodec.H264,
    val colorSpace: ClientColorSpace = ClientColorSpace.Rec709,
    val maxFps: Int = 60,
    val targetBitrateKbps: Int = 18_000,
    val keyframeIntervalMs: Int = 1_000,
    val lowLatency: Boolean = true,
) {
    val width: Int
        get() = resolution.width.takeIf { it > 0 } ?: 1920

    val height: Int
        get() = resolution.height.takeIf { it > 0 } ?: 1080

    val summary: String
        get() = "${resolution.label} / ${maxFps} fps / ${targetBitrateKbps} kbps / ${codec.label} / ${colorSpace.label}"
}

data class ClientInputSettings(
    val touchMode: ClientTouchMode = ClientTouchMode.Direct,
    val bluetoothKeyboardEnabled: Boolean = true,
    val bluetoothMouseEnabled: Boolean = true,
    val gameControllerEnabled: Boolean = true,
    val fullscreenMode: Boolean = false,
    val specialKeyOverlay: Boolean = true,
)

data class ClientSessionPreferences(
    val videoSettings: ClientVideoSettings = ClientVideoSettings(),
    val inputSettings: ClientInputSettings = ClientInputSettings(),
)

class AndroidSessionPreferencesStore(context: Context) {
    private val preferences = context.applicationContext.getSharedPreferences(
        "glyphray_session_preferences",
        Context.MODE_PRIVATE,
    )

    fun load(): ClientSessionPreferences {
        val video = ClientVideoSettings(
            displayId = preferences.getInt(keyDisplayId, 0),
            resolution = preferences.enumValue(keyResolution, ClientResolution.R1080p),
            codec = preferences.enumValue(keyCodec, ClientVideoCodec.H264),
            colorSpace = preferences.enumValue(keyColorSpace, ClientColorSpace.Rec709),
            maxFps = preferences.getInt(keyMaxFps, 60).coerceIn(30, 120),
            targetBitrateKbps = preferences.getInt(keyBitrate, 18_000).coerceIn(4_000, 120_000),
            keyframeIntervalMs = preferences.getInt(keyKeyframeInterval, 1_000).coerceIn(250, 10_000),
            lowLatency = preferences.getBoolean(keyLowLatency, true),
        )
        val input = ClientInputSettings(
            touchMode = preferences.enumValue(keyTouchMode, ClientTouchMode.Direct),
            bluetoothKeyboardEnabled = preferences.getBoolean(keyBluetoothKeyboard, true),
            bluetoothMouseEnabled = preferences.getBoolean(keyBluetoothMouse, true),
            gameControllerEnabled = preferences.getBoolean(keyGameController, true),
            fullscreenMode = preferences.getBoolean(keyFullscreen, false),
            specialKeyOverlay = preferences.getBoolean(keySpecialKeys, true),
        )
        return ClientSessionPreferences(video, input)
    }

    fun saveVideoSettings(settings: ClientVideoSettings) {
        preferences.edit()
            .putInt(keyDisplayId, settings.displayId)
            .putString(keyResolution, settings.resolution.name)
            .putString(keyCodec, settings.codec.name)
            .putString(keyColorSpace, settings.colorSpace.name)
            .putInt(keyMaxFps, settings.maxFps)
            .putInt(keyBitrate, settings.targetBitrateKbps)
            .putInt(keyKeyframeInterval, settings.keyframeIntervalMs)
            .putBoolean(keyLowLatency, settings.lowLatency)
            .apply()
    }

    fun saveInputSettings(settings: ClientInputSettings) {
        preferences.edit()
            .putString(keyTouchMode, settings.touchMode.name)
            .putBoolean(keyBluetoothKeyboard, settings.bluetoothKeyboardEnabled)
            .putBoolean(keyBluetoothMouse, settings.bluetoothMouseEnabled)
            .putBoolean(keyGameController, settings.gameControllerEnabled)
            .putBoolean(keyFullscreen, settings.fullscreenMode)
            .putBoolean(keySpecialKeys, settings.specialKeyOverlay)
            .apply()
    }

    private companion object {
        const val keyDisplayId = "display_id"
        const val keyResolution = "resolution"
        const val keyCodec = "codec"
        const val keyColorSpace = "color_space"
        const val keyMaxFps = "max_fps"
        const val keyBitrate = "bitrate_kbps"
        const val keyKeyframeInterval = "keyframe_interval_ms"
        const val keyLowLatency = "low_latency"
        const val keyTouchMode = "touch_mode"
        const val keyBluetoothKeyboard = "bluetooth_keyboard"
        const val keyBluetoothMouse = "bluetooth_mouse"
        const val keyGameController = "game_controller"
        const val keyFullscreen = "fullscreen"
        const val keySpecialKeys = "special_keys"
    }
}

private inline fun <reified T : Enum<T>> android.content.SharedPreferences.enumValue(
    key: String,
    default: T,
): T {
    val raw = getString(key, null) ?: return default
    return runCatching { enumValueOf<T>(raw) }.getOrDefault(default)
}
