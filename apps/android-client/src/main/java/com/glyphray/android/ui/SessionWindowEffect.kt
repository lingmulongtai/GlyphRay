package com.glyphray.android.ui

import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.os.Build
import android.view.View
import android.view.Window
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.platform.LocalContext

@Composable
fun SessionWindowEffect(active: Boolean, immersive: Boolean) {
    val activity = LocalContext.current.findActivity()
    DisposableEffect(activity, active, immersive) {
        val window = activity?.window
        if (window != null) {
            if (active) {
                window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            } else {
                window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            }

            if (immersive) {
                window.enterGlyphRayImmersiveMode()
            } else {
                window.exitGlyphRayImmersiveMode()
            }
        }

        onDispose {
            window?.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            window?.exitGlyphRayImmersiveMode()
        }
    }
}

private fun Context.findActivity(): Activity? {
    var current: Context? = this
    while (current is ContextWrapper) {
        if (current is Activity) {
            return current
        }
        current = current.baseContext
    }
    return null
}

private fun Window.enterGlyphRayImmersiveMode() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        insetsController?.let { controller ->
            controller.hide(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
            controller.systemBarsBehavior =
                WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    } else {
        @Suppress("DEPRECATION")
        decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY or
            View.SYSTEM_UI_FLAG_FULLSCREEN or
            View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or
            View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or
            View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION or
            View.SYSTEM_UI_FLAG_LAYOUT_STABLE
    }
}

private fun Window.exitGlyphRayImmersiveMode() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        insetsController?.show(WindowInsets.Type.statusBars() or WindowInsets.Type.navigationBars())
    } else {
        @Suppress("DEPRECATION")
        decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_VISIBLE
    }
}
