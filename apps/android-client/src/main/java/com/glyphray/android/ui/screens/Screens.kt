package com.glyphray.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AssistChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInteropFilter
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import android.view.KeyEvent
import com.glyphray.android.network.DiscoveredHost
import com.glyphray.android.input.StylusDiagnosticsController
import com.glyphray.android.network.ClientColorSpace
import com.glyphray.android.network.ClientInputSettings
import com.glyphray.android.network.ClientResolution
import com.glyphray.android.network.ClientTouchMode
import com.glyphray.android.network.ClientVideoCodec
import com.glyphray.android.network.ClientVideoSettings
import com.glyphray.android.network.HostDiscoveryState
import com.glyphray.android.network.SessionControlState
import com.glyphray.android.network.SpecialRemoteKey
import com.glyphray.android.network.StylusLanBridgeController
import com.glyphray.android.ui.components.CalibrationPanel
import com.glyphray.android.ui.components.CalibrationStep
import com.glyphray.android.ui.components.SessionTelemetrySnapshot
import com.glyphray.android.ui.MetricRow
import com.glyphray.android.ui.PrimaryAction
import com.glyphray.android.ui.ScreenFrame
import com.glyphray.android.ui.ToggleRow
import com.glyphray.android.ui.video.RemoteDisplayView

@Composable
fun HostListScreen(
    discoveryState: HostDiscoveryState,
    onRefresh: () -> Unit,
    onAddManualHost: (String) -> Unit,
    onPair: () -> Unit,
    onConnect: (DiscoveredHost) -> Unit,
) {
    var manualHost by remember { mutableStateOf("") }
    ScreenFrame(
        title = "GlyphRay",
        subtitle = "Creative remote display hosts on your local network",
        actions = {
            PrimaryAction("Scan", onRefresh)
            PrimaryAction("Pair", onPair)
        },
    ) {
        if (discoveryState.hosts.isEmpty()) {
            Text(
                text = if (discoveryState.isScanning) {
                    "Listening for GlyphRay hosts..."
                } else {
                    "No GlyphRay hosts found"
                },
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            TextField(
                value = manualHost,
                onValueChange = { manualHost = it },
                modifier = Modifier.weight(1f),
                singleLine = true,
                label = { Text("Host IP / Tailscale") },
            )
            PrimaryAction("Add") {
                if (manualHost.isNotBlank()) {
                    onAddManualHost(manualHost)
                }
            }
        }
        Spacer(Modifier.height(14.dp))

        discoveryState.hosts.forEach { host ->
            HostRow(
                name = host.hostName,
                details = "${host.address.hostAddress}:${host.controlPort} / ${host.capabilitiesLabel} / load ${host.loadPercent}%",
                onConnect = { onConnect(host) },
            )
        }
        Spacer(Modifier.height(18.dp))
        discoveryState.lastError?.let { error ->
            Text("Discovery error: $error", color = MaterialTheme.colorScheme.error)
        }
        Text(
            "Last scan: ${discoveryState.lastScanLabel}",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun HostRow(name: String, details: String, onConnect: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 10.dp)
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.3f), RoundedCornerShape(8.dp))
            .padding(14.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(name, fontWeight = FontWeight.SemiBold)
            Text(details, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
        PrimaryAction("Connect", onConnect)
    }
}

@Composable
fun PairingScreen(onDone: () -> Unit) {
    ScreenFrame(
        title = "Pair Computer",
        subtitle = "Local-network pairing with numeric code or QR handoff",
    ) {
        MetricRow("Pairing method", "Numeric code first")
        MetricRow("Session security", "Mutual authentication")
        MetricRow("Secret storage", "Android Keystore")
        Spacer(Modifier.height(18.dp))
        PrimaryAction("Trust this host", onDone)
    }
}

@Composable
fun ConnectionScreen(
    selectedHost: DiscoveredHost?,
    controlState: SessionControlState,
    onConnected: () -> Unit,
) {
    ScreenFrame(
        title = "Connect",
        subtitle = "Permission, display, and encoder negotiation",
    ) {
        MetricRow("Host", selectedHost?.hostName ?: "No host selected")
        MetricRow("Endpoint", selectedHost?.let { "${it.address.hostAddress}:${it.controlPort}" } ?: "-")
        MetricRow("Selected display", controlState.primaryDisplay?.label ?: "Primary monitor")
        MetricRow("Video", "H.264 / low latency / 60 fps")
        MetricRow("Input", "Stylus priority channel")
        MetricRow("Control", controlState.statusLabel)
        MetricRow("Control packets", controlState.packetsSent.toString())
        MetricRow("Control responses", controlState.responsesReceived.toString())
        MetricRow("Pairing", controlState.lastPairingAccepted?.let { if (it) "Accepted" else "Rejected" } ?: "Waiting")
        MetricRow("Latency pong", controlState.lastRoundTripMs?.let { "${it} ms" } ?: "-")
        MetricRow("Host displays", controlState.displays.size.toString())
        MetricRow("Trusted device", controlState.trustedDeviceId ?: "-")
        MetricRow("Last control action", controlState.lastAction)
        controlState.lastError?.let { error ->
            Text("Control error: $error", color = MaterialTheme.colorScheme.error)
        }
        Spacer(Modifier.height(18.dp))
        PrimaryAction("Start session", onConnected)
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun RemoteSessionScreen(
    selectedHost: DiscoveredHost?,
    controlState: SessionControlState,
    fullscreen: Boolean,
    onFullscreenChange: (Boolean) -> Unit,
    onVideoSettings: () -> Unit,
    onSpecialKey: (SpecialRemoteKey) -> Unit,
    onKeyEvent: (KeyEvent) -> Boolean,
    onGenericMotionEvent: (android.view.MotionEvent) -> Boolean,
    onPenSettings: () -> Unit,
    onDiagnostics: () -> Unit,
) {
    val stylusBridge = remember { StylusLanBridgeController() }
    val bridgeState = stylusBridge.state

    androidx.compose.runtime.DisposableEffect(selectedHost) {
        if (selectedHost != null) {
            stylusBridge.connect(selectedHost)
        } else {
            stylusBridge.disconnect()
        }
        onDispose { stylusBridge.disconnect() }
    }

    androidx.compose.runtime.DisposableEffect(stylusBridge) {
        onDispose { stylusBridge.close() }
    }

    ScreenFrame(
        title = "Session",
        subtitle = selectedHost?.let { "Streaming workspace for ${it.hostName}" }
            ?: "Low-latency remote display workspace",
        actions = {
            PrimaryAction("Video", onVideoSettings)
            PrimaryAction("Pen", onPenSettings)
            PrimaryAction("Diag", onDiagnostics)
            PrimaryAction(if (fullscreen) "Window" else "Full") {
                onFullscreenChange(!fullscreen)
            }
        },
    ) {
        RemoteDisplayView(
            telemetry = SessionTelemetrySnapshot(
                roundTripMs = 12,
                decodeMs = 3,
                renderMs = 2,
                inputMs = 4,
                fps = 60,
                bitrateKbps = 18_000,
            ),
            modifier = Modifier
                .fillMaxWidth()
                .height(if (fullscreen) 620.dp else 320.dp),
            onInputEvent = { event -> stylusBridge.onMotionEvent(event, controlState.inputSettings) },
            onKeyEvent = onKeyEvent,
            onGenericMotionEvent = onGenericMotionEvent,
        )
        Spacer(Modifier.height(14.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            AssistChip(onClick = {}, label = { Text("60 fps") })
            AssistChip(onClick = {}, label = { Text("12 ms RTT") })
            AssistChip(onClick = {}, label = { Text("Ink input") })
        }
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (controlState.inputSettings.specialKeyOverlay) {
                AssistChip(onClick = { onSpecialKey(SpecialRemoteKey.Windows) }, label = { Text("Win") })
                AssistChip(onClick = { onSpecialKey(SpecialRemoteKey.PrintScreen) }, label = { Text("PrtSc") })
            }
            AssistChip(onClick = {}, label = { Text(controlState.inputSettings.touchMode.label) })
            AssistChip(onClick = {}, label = { Text("Mouse") })
            AssistChip(onClick = {}, label = { Text("Gamepad") })
        }
        Spacer(Modifier.height(10.dp))
        MetricRow("Input stream", bridgeState.statusLabel)
        MetricRow("Input host", bridgeState.connectedHostName ?: "-")
        MetricRow("Bluetooth keyboard", if (controlState.inputSettings.bluetoothKeyboardEnabled) "Enabled" else "Off")
        MetricRow("Fullscreen", if (fullscreen) "On" else "Off")
        MetricRow("Stylus packets", bridgeState.packetsSent.toString())
        MetricRow("Stylus samples", bridgeState.samplesSent.toString())
        bridgeState.lastError?.let { error ->
            Text("Input stream error: $error", color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
fun PenSettingsScreen(onDiagnostics: () -> Unit) {
    var pressure by remember { mutableFloatStateOf(0.45f) }
    var palmRejection by remember { mutableStateOf(true) }
    var calibrationStep by remember { mutableStateOf(CalibrationStep.TopLeft) }

    ScreenFrame(
        title = "Pen Settings",
        subtitle = "Pressure curves, calibration, and mapping",
        actions = { PrimaryAction("Raw input", onDiagnostics) },
    ) {
        Text("Pressure curve", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            AssistChip(onClick = {}, label = { Text("Linear") })
            AssistChip(onClick = {}, label = { Text("Soft") })
            AssistChip(onClick = {}, label = { Text("Hard") })
        }
        Spacer(Modifier.height(18.dp))
        Text("Test pressure: ${(pressure * 100).toInt()}%")
        Slider(value = pressure, onValueChange = { pressure = it })
        ToggleRow("Palm rejection hints", palmRejection) { palmRejection = it }
        Spacer(Modifier.height(12.dp))
        MetricRow("Mapping", "Fit / Fill / 1:1 / selected monitor")
        MetricRow("Calibration", calibrationStep.name)
        Spacer(Modifier.height(12.dp))
        CalibrationPanel(step = calibrationStep)
        Spacer(Modifier.height(12.dp))
        PrimaryAction("Advance calibration") {
            calibrationStep = when (calibrationStep) {
                CalibrationStep.TopLeft -> CalibrationStep.BottomRight
                CalibrationStep.BottomRight -> CalibrationStep.Complete
                CalibrationStep.Complete -> CalibrationStep.TopLeft
            }
        }
    }
}

@Composable
fun VideoSettingsScreen(
    controlState: SessionControlState,
    onSettingsChange: (ClientVideoSettings) -> Unit,
    onInputSettingsChange: (ClientInputSettings) -> Unit,
    onApply: () -> Unit,
) {
    val settings = controlState.videoSettings
    val input = controlState.inputSettings

    ScreenFrame(
        title = "Video Settings",
        subtitle = "Client-requested encoder and session controls",
        actions = { PrimaryAction("Apply", onApply) },
    ) {
        MetricRow("Current request", settings.summary)
        Spacer(Modifier.height(12.dp))
        Text("Resolution", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ClientResolution.entries.forEach { resolution ->
                AssistChip(
                    onClick = { onSettingsChange(settings.copy(resolution = resolution)) },
                    label = { Text(resolution.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        Text("Codec", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ClientVideoCodec.entries.forEach { codec ->
                AssistChip(
                    onClick = { onSettingsChange(settings.copy(codec = codec)) },
                    label = { Text(codec.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        Text("Color space", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ClientColorSpace.entries.forEach { colorSpace ->
                AssistChip(
                    onClick = { onSettingsChange(settings.copy(colorSpace = colorSpace)) },
                    label = { Text(colorSpace.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        MetricRow("Refresh rate", "${settings.maxFps} fps")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(60, 90, 120).forEach { fps ->
                AssistChip(onClick = { onSettingsChange(settings.copy(maxFps = fps)) }, label = { Text("$fps") })
            }
        }
        Spacer(Modifier.height(12.dp))
        MetricRow("Bitrate", "${settings.targetBitrateKbps} kbps")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(12_000, 18_000, 35_000, 60_000).forEach { bitrate ->
                AssistChip(
                    onClick = { onSettingsChange(settings.copy(targetBitrateKbps = bitrate)) },
                    label = { Text("${bitrate / 1_000} Mbps") },
                )
            }
        }
        Spacer(Modifier.height(18.dp))
        Text("Client controls", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ClientTouchMode.entries.forEach { touchMode ->
                AssistChip(
                    onClick = { onInputSettingsChange(input.copy(touchMode = touchMode)) },
                    label = { Text(touchMode.label) },
                )
            }
        }
        Spacer(Modifier.height(8.dp))
        ToggleRow("Bluetooth keyboard capture", input.bluetoothKeyboardEnabled) {
            onInputSettingsChange(input.copy(bluetoothKeyboardEnabled = it))
        }
        ToggleRow("Bluetooth mouse capture", input.bluetoothMouseEnabled) {
            onInputSettingsChange(input.copy(bluetoothMouseEnabled = it))
        }
        ToggleRow("Game controller capture", input.gameControllerEnabled) {
            onInputSettingsChange(input.copy(gameControllerEnabled = it))
        }
        ToggleRow("Special key overlay", input.specialKeyOverlay) {
            onInputSettingsChange(input.copy(specialKeyOverlay = it))
        }
        MetricRow("Render target", "Low-latency surface")
    }
}

@Composable
fun SecuritySettingsScreen() {
    ScreenFrame(
        title = "Security",
        subtitle = "Local trust, pairing, and device identity",
    ) {
        MetricRow("Device identity", "Android Keystore")
        MetricRow("Pairing", "One-time code / QR-ready")
        MetricRow("Transport", "Encrypted session packets")
        MetricRow("Keyboard logging", "Disabled")
        MetricRow("Clipboard", "Off")
    }
}

@OptIn(ExperimentalComposeUiApi::class)
@Composable
fun DiagnosticsScreen(controller: StylusDiagnosticsController) {
    val state = controller.state
    val latest = state.latest

    ScreenFrame(
        title = "Stylus Diagnostics",
        subtitle = "Raw Samsung S Pen and Android MotionEvent values",
    ) {
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .height(260.dp)
                .background(Color(0xFF070A0E), RoundedCornerShape(8.dp))
                .border(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.5f), RoundedCornerShape(8.dp))
                .pointerInteropFilter(onTouchEvent = controller::onMotionEvent),
            contentAlignment = Alignment.Center,
        ) {
            Text("Draw or hover here", color = Color(0xFF9DB2BC))
        }
        Spacer(Modifier.height(16.dp))
        MetricRow("Total samples", state.totalSamples.toString())
        MetricRow("Historical samples", state.historicalSamples.toString())
        MetricRow("Hover samples", state.hoverSamples.toString())
        MetricRow("Contact samples", state.contactSamples.toString())
        MetricRow("Last batch size", state.lastBatchSize.toString())
        Spacer(Modifier.height(8.dp))
        MetricRow("Tool", latest?.toolType?.name ?: "-")
        MetricRow("Action", latest?.action?.name ?: "-")
        MetricRow("Pointer id", latest?.pointerId?.toString() ?: "-")
        MetricRow("X / Y", latest?.let { "%.1f / %.1f".format(it.x, it.y) } ?: "-")
        MetricRow("Pressure", latest?.let { "%.4f".format(it.pressure) } ?: "-")
        MetricRow("Tilt", latest?.let { "%.2f deg".format(it.tiltDegrees) } ?: "-")
        MetricRow("Orientation", latest?.let { "%.2f deg".format(it.orientationDegrees) } ?: "-")
        MetricRow("Distance", latest?.let { "%.4f".format(it.distance) } ?: "-")
        MetricRow("Buttons", latest?.buttonState?.toString() ?: "-")
        MetricRow("Eraser", latest?.isEraser?.toString() ?: "-")
        MetricRow("Timestamp ns", latest?.eventTimeNanos?.toString() ?: "-")
    }
}
