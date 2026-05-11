package com.glyphray.android.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

data class SessionTelemetrySnapshot(
    val roundTripMs: Int = 0,
    val decodeMs: Int = 0,
    val renderMs: Int = 0,
    val inputMs: Int = 0,
    val fps: Int = 0,
    val bitrateKbps: Int = 0,
)

@Composable
fun LatencyOverlay(
    telemetry: SessionTelemetrySnapshot,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier,
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.86f),
        shape = androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            OverlayMetric("RTT", "${telemetry.roundTripMs} ms")
            OverlayMetric("Decode", "${telemetry.decodeMs} ms")
            OverlayMetric("Render", "${telemetry.renderMs} ms")
            OverlayMetric("Input", "${telemetry.inputMs} ms")
            OverlayMetric("FPS", telemetry.fps.toString())
            OverlayMetric("Video", "${telemetry.bitrateKbps} kbps")
        }
    }
}

@Composable
private fun OverlayMetric(label: String, value: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, fontWeight = FontWeight.Medium)
    }
}

