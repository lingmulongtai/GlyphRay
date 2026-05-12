package com.glyphray.android.ui.video

import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.MotionEvent
import android.view.KeyEvent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import com.glyphray.android.ui.components.LatencyOverlay
import com.glyphray.android.ui.components.SessionTelemetrySnapshot
import com.glyphray.android.video.RemoteVideoDecoder
import com.glyphray.android.video.RemoteVideoStreamController
import com.glyphray.android.video.VideoDecoderConfig

@Composable
fun RemoteDisplayView(
    telemetry: SessionTelemetrySnapshot,
    modifier: Modifier = Modifier,
    onInputEvent: ((MotionEvent) -> Boolean)? = null,
    onKeyEvent: ((KeyEvent) -> Boolean)? = null,
    onGenericMotionEvent: ((MotionEvent) -> Boolean)? = null,
) {
    var status by remember { mutableStateOf("Waiting for video") }
    var decoder by remember { mutableStateOf<RemoteVideoDecoder?>(null) }
    val streamController = remember { RemoteVideoStreamController() }

    DisposableEffect(Unit) {
        onDispose {
            decoder?.close()
            streamController.detachDecoder()
            decoder = null
        }
    }

    Box(
        modifier = modifier
            .background(Color(0xFF070A0E)),
    ) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                SurfaceView(context).apply {
                    isFocusable = true
                    isFocusableInTouchMode = true
                    setBackgroundColor(android.graphics.Color.BLACK)
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) {
                            decoder?.close()
                            decoder = RemoteVideoDecoder(holder.surface)
                            runCatching {
                                decoder?.configure(VideoDecoderConfig(width = 1920, height = 1080))
                                decoder?.let(streamController::attachDecoder)
                            }.onSuccess {
                                status = "Decoder ready"
                            }.onFailure { error ->
                                status = error.message ?: "Decoder unavailable"
                            }
                        }

                        override fun surfaceChanged(
                            holder: SurfaceHolder,
                            format: Int,
                            width: Int,
                            height: Int,
                        ) = Unit

                        override fun surfaceDestroyed(holder: SurfaceHolder) {
                            decoder?.close()
                            streamController.detachDecoder()
                            decoder = null
                            status = "Surface closed"
                        }
                    })
                }
            },
            update = { view ->
                view.requestFocus()
                view.setOnTouchListener { _, event ->
                    onInputEvent?.invoke(event) ?: false
                }
                view.setOnKeyListener { _, _, event ->
                    onKeyEvent?.invoke(event) ?: false
                }
                view.setOnGenericMotionListener { _, event ->
                    onGenericMotionEvent?.invoke(event) ?: false
                }
            },
        )
        LatencyOverlay(
            telemetry = telemetry,
            modifier = Modifier
                .align(Alignment.TopEnd)
                .padding(12.dp),
        )
        Text(
            text = status,
            modifier = Modifier
                .align(Alignment.BottomStart)
                .padding(12.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
