package com.glyphray.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.unit.dp

private val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFF6EE7F9),
    onPrimary = Color(0xFF041216),
    secondary = Color(0xFFF4D35E),
    onSecondary = Color(0xFF201A02),
    background = Color(0xFF111317),
    onBackground = Color(0xFFE6EDF2),
    surface = Color(0xFF171B21),
    onSurface = Color(0xFFE6EDF2),
    surfaceVariant = Color(0xFF222832),
    onSurfaceVariant = Color(0xFFA8B6C0),
    outline = Color(0xFF3D4852),
    tertiary = Color(0xFFB8F5D6),
    error = Color(0xFFFFB4AB),
)

private val GlyphRayShapes = Shapes(
    extraSmall = RoundedCornerShape(4.dp),
    small = RoundedCornerShape(6.dp),
    medium = RoundedCornerShape(8.dp),
    large = RoundedCornerShape(8.dp),
    extraLarge = RoundedCornerShape(8.dp),
)

@Composable
fun GlyphRayTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = DarkScheme,
        shapes = GlyphRayShapes,
        typography = androidx.compose.material3.Typography(),
        content = content,
    )
}
