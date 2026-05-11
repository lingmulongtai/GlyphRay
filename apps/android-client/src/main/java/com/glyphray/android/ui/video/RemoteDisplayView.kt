package com.glyphray.android.ui.video

import android.view.SurfaceHolder
import android.view.SurfaceView
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
import com.glyphray.android.video.VideoDecoderConfig

@Composable
fun RemoteDisplayView(
    telemetry: SessionTelemetrySnapshot,
    modifier: Modifier = Modifier,
) {
    var status by remember { mutableStateOf("Waiting for video") }
    var decoder by remember { mutableStateOf<RemoteVideoDecoder?>(null) }

    DisposableEffect(Unit) {
        onDispose {
            decoder?.close()
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
                    setBackgroundColor(android.graphics.Color.BLACK)
                    holder.addCallback(object : SurfaceHolder.Callback {
                        override fun surfaceCreated(holder: SurfaceHolder) {
                            decoder?.close()
                            decoder = RemoteVideoDecoder(holder.surface)
                            runCatching {
                                decoder?.configure(VideoDecoderConfig(width = 1920, height = 1080))
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
                            decoder = null
                            status = "Surface closed"
                        }
                    })
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

