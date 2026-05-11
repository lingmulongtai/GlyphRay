package com.glyphray.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFF6EE7F9),
    onPrimary = Color(0xFF041216),
    secondary = Color(0xFFF4D35E),
    onSecondary = Color(0xFF201A02),
    background = Color(0xFF111317),
    onBackground = Color(0xFFE6EDF2),
    surface = Color(0xFF171B21),
    onSurface = Color(0xFFE6EDF2),
    onSurfaceVariant = Color(0xFF9DB2BC),
    outline = Color(0xFF3D4852),
)

@Composable
fun GlyphRayTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = DarkScheme,
        typography = androidx.compose.material3.Typography(),
        content = content,
    )
}

