package com.glyphray.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import com.glyphray.android.ui.GlyphRayApp
import com.glyphray.android.ui.theme.GlyphRayTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            GlyphRayTheme {
                GlyphRayApp()
            }
        }
    }
}

