package com.glyphray.android.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AssistChip
import androidx.compose.material3.FilterChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
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
import androidx.compose.ui.unit.sp
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
import com.glyphray.android.network.SessionRealtimeInputSender
import com.glyphray.android.ui.components.CalibrationPanel
import com.glyphray.android.ui.components.CalibrationStep
import com.glyphray.android.ui.components.SessionTelemetrySnapshot
import com.glyphray.android.ui.MetricRow
import com.glyphray.android.ui.PrimaryAction
import com.glyphray.android.ui.ScreenFrame
import com.glyphray.android.ui.ToggleRow
import com.glyphray.android.ui.video.RemoteDisplayView
import com.glyphray.android.video.RemoteVideoStreamController

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
        subtitle = "Remote creative workstations",
        actions = {
            PrimaryAction("Scan", onRefresh)
            PrimaryAction("Pair", onPair)
        },
    ) {
        StatusBand(
            items = listOf(
                StatusMetric("Hosts", discoveryState.hosts.size.toString(), Tone.Primary),
                StatusMetric("Discovery", if (discoveryState.isScanning) "Scanning" else "Idle", if (discoveryState.isScanning) Tone.Good else Tone.Neutral),
                StatusMetric("Last scan", discoveryState.lastScanLabel, Tone.Neutral),
            ),
        )
        Spacer(Modifier.height(16.dp))

        if (discoveryState.hosts.isEmpty()) {
            InfoPanel {
                Text(
                    text = if (discoveryState.isScanning) "Listening for GlyphRay hosts" else "No hosts discovered",
                    fontWeight = FontWeight.SemiBold,
                )
                Text(
                    text = "Manual endpoint entry stays available for Tailscale IPs, MagicDNS names, and direct LAN addresses.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Spacer(Modifier.height(14.dp))
        }

        SectionTitle("Manual endpoint")
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
                    manualHost = ""
                }
            }
        }
        Spacer(Modifier.height(18.dp))

        SectionTitle("Available hosts")
        discoveryState.hosts.forEach { host ->
            HostCard(host = host, onConnect = { onConnect(host) })
            Spacer(Modifier.height(10.dp))
        }
        Spacer(Modifier.height(8.dp))
        discoveryState.lastError?.let { error ->
            Text("Discovery error: $error", color = MaterialTheme.colorScheme.error)
        }
    }
}

@Composable
private fun HostCard(host: DiscoveredHost, onConnect: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.3f), RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.72f), RoundedCornerShape(8.dp))
            .padding(14.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.Top,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(host.hostName, fontWeight = FontWeight.SemiBold)
            Text("${host.address.hostAddress}:${host.controlPort}", color = MaterialTheme.colorScheme.onSurfaceVariant)
            Spacer(Modifier.height(8.dp))
            ChipRow {
                StatusPill(if (host.supportsWindowsInk) "Ink ready" else "Mouse fallback", if (host.supportsWindowsInk) Tone.Good else Tone.Warning)
                StatusPill(if (host.supportsH264) "H.264" else "No codec", if (host.supportsH264) Tone.Good else Tone.Warning)
                StatusPill(if (host.pairingRequired) "Pairing" else "Trusted", if (host.pairingRequired) Tone.Warning else Tone.Good)
                StatusPill("Load ${host.loadPercent}%", if (host.loadPercent < 70) Tone.Neutral else Tone.Warning)
            }
        }
        PrimaryAction("Connect", onConnect)
    }
}

@Composable
fun PairingScreen(onDone: () -> Unit) {
    ScreenFrame(
        title = "Pair Computer",
        subtitle = "Local trust setup",
    ) {
        StatusBand(
            items = listOf(
                StatusMetric("Method", "Code / QR-ready", Tone.Primary),
                StatusMetric("Auth", "Mutual", Tone.Good),
                StatusMetric("Secrets", "Keystore", Tone.Good),
            ),
        )
        Spacer(Modifier.height(18.dp))
        InfoPanel {
            SectionTitle("Trust model")
            MetricRow("Pairing token", "One-time")
            MetricRow("Session token", "Short-lived")
            MetricRow("Raw keyboard logs", "Disabled")
        }
        Spacer(Modifier.height(14.dp))
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
        subtitle = "Session readiness and stream negotiation",
    ) {
        StatusBand(
            items = listOf(
                StatusMetric("Control", if (controlState.isConnected) "Ready" else "Offline", if (controlState.isConnected) Tone.Good else Tone.Warning),
                StatusMetric("Pairing", controlState.lastPairingAccepted?.let { if (it) "Accepted" else "Rejected" } ?: "Pending", if (controlState.lastPairingAccepted == true) Tone.Good else Tone.Neutral),
                StatusMetric("RTT", controlState.lastRoundTripMs?.let { "${it} ms" } ?: "-", Tone.Primary),
                StatusMetric("Displays", controlState.displays.size.toString(), Tone.Neutral),
            ),
        )
        Spacer(Modifier.height(16.dp))

        InfoPanel {
            SectionTitle("Target")
            MetricRow("Host", selectedHost?.hostName ?: "No host selected")
            MetricRow("Endpoint", selectedHost?.let { "${it.address.hostAddress}:${it.controlPort}" } ?: "-")
            MetricRow("Selected display", controlState.primaryDisplay?.label ?: "Primary monitor")
        }
        Spacer(Modifier.height(12.dp))

        InfoPanel {
            SectionTitle("Requested session")
            MetricRow("Video", controlState.videoSettings.summary)
            MetricRow("Input", "${controlState.inputSettings.touchMode.label} / stylus priority")
            MetricRow("Fullscreen", if (controlState.inputSettings.fullscreenMode) "On" else "Windowed")
            MetricRow("Trusted device", controlState.trustedDeviceId ?: "-")
            MetricRow("Session security", if (controlState.secureSession) "AES-256-GCM" else "Negotiating")
        }
        Spacer(Modifier.height(12.dp))

        InfoPanel {
            SectionTitle("Readiness")
            ReadinessRow("Host selected", selectedHost != null)
            ReadinessRow("Control channel", controlState.isConnected)
            ReadinessRow("Pairing accepted", controlState.lastPairingAccepted == true)
            ReadinessRow("Encrypted session", controlState.secureSession)
            ReadinessRow("Display geometry", controlState.displays.isNotEmpty())
            ReadinessRow("Encoder request", controlState.videoSettings.lowLatency)
        }
        Spacer(Modifier.height(12.dp))
        MetricRow("Control packets", controlState.packetsSent.toString())
        MetricRow("Control responses", controlState.responsesReceived.toString())
        MetricRow("Last control action", controlState.lastAction)
        controlState.lastError?.let { error ->
            Text("Control error: $error", color = MaterialTheme.colorScheme.error)
        }
        Spacer(Modifier.height(18.dp))
        PrimaryAction("Start session", enabled = selectedHost != null, onClick = onConnected)
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
    onVideoStreamController: (RemoteVideoStreamController?) -> Unit,
    realtimeInputSender: SessionRealtimeInputSender,
    onPenSettings: () -> Unit,
    onDiagnostics: () -> Unit,
) {
    val stylusBridge = remember(realtimeInputSender) {
        StylusLanBridgeController(realtimeInputSender)
    }
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
        StatusBand(
            items = listOf(
                StatusMetric("Host", selectedHost?.hostName ?: "None", if (selectedHost != null) Tone.Good else Tone.Warning),
                StatusMetric("RTT", controlState.lastRoundTripMs?.let { "${it} ms" } ?: "-", Tone.Primary),
                StatusMetric("Stream", "${controlState.videoSettings.maxFps} fps", Tone.Neutral),
                StatusMetric("Input", bridgeState.statusLabel, if (bridgeState.lastError == null) Tone.Good else Tone.Warning),
            ),
        )
        Spacer(Modifier.height(14.dp))

        RemoteDisplayView(
            telemetry = SessionTelemetrySnapshot(
                roundTripMs = controlState.lastRoundTripMs?.toInt() ?: 0,
                decodeMs = 0,
                renderMs = 0,
                inputMs = 0,
                fps = controlState.videoSettings.maxFps,
                bitrateKbps = controlState.videoSettings.targetBitrateKbps,
            ),
            modifier = Modifier
                .fillMaxWidth()
                .height(if (fullscreen) 620.dp else 360.dp),
            onInputEvent = { event ->
                stylusBridge.onMotionEvent(
                    event = event,
                    inputSettings = controlState.inputSettings,
                    displayId = controlState.videoSettings.displayId,
                )
            },
            onKeyEvent = onKeyEvent,
            onGenericMotionEvent = onGenericMotionEvent,
            onVideoStreamController = onVideoStreamController,
        )
        Spacer(Modifier.height(14.dp))
        ChipRow {
            StatusPill(controlState.videoSettings.codec.label, Tone.Primary)
            StatusPill(controlState.videoSettings.colorSpace.label, Tone.Neutral)
            StatusPill("${controlState.videoSettings.targetBitrateKbps / 1_000} Mbps", Tone.Neutral)
            StatusPill("Ink input", Tone.Good)
            StatusPill(controlState.inputSettings.touchMode.label, Tone.Neutral)
            StatusPill(if (fullscreen) "Fullscreen" else "Windowed", Tone.Neutral)
            if (controlState.inputSettings.specialKeyOverlay) {
                AssistChip(onClick = { onSpecialKey(SpecialRemoteKey.Windows) }, label = { Text("Win") })
                AssistChip(onClick = { onSpecialKey(SpecialRemoteKey.PrintScreen) }, label = { Text("PrtSc") })
            }
        }
        Spacer(Modifier.height(14.dp))

        Column(
            modifier = Modifier.fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            InfoPanel {
                SectionTitle("Input")
                MetricRow("Stream", bridgeState.statusLabel)
                MetricRow("Host", bridgeState.connectedHostName ?: "-")
                MetricRow("Stylus packets", bridgeState.packetsSent.toString())
                MetricRow("Stylus samples", bridgeState.samplesSent.toString())
                MetricRow("Keyboard", if (controlState.inputSettings.bluetoothKeyboardEnabled) "Enabled" else "Off")
                MetricRow("Mouse", if (controlState.inputSettings.bluetoothMouseEnabled) "Enabled" else "Off")
                MetricRow("Gamepad", if (controlState.inputSettings.gameControllerEnabled) "Enabled" else "Off")
            }
            InfoPanel {
                SectionTitle("Video")
                MetricRow("Packets", controlState.videoPacketsReceived.toString())
                MetricRow("Frames", controlState.videoFramesCompleted.toString())
                MetricRow("Decoder queued", controlState.videoFramesQueuedToDecoder.toString())
                MetricRow("Last sequence", controlState.lastVideoSequence?.toString() ?: "-")
                MetricRow("Request", controlState.videoSettings.summary)
            }
        }
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
    var pressureCurve by remember { mutableStateOf("Linear") }
    var mappingMode by remember { mutableStateOf("Fit") }

    ScreenFrame(
        title = "Pen Settings",
        subtitle = "Pressure curves, calibration, and mapping",
        actions = { PrimaryAction("Raw input", onDiagnostics) },
    ) {
        InfoPanel {
            SectionTitle("Pressure")
            ChipRow {
                listOf("Linear", "Soft", "Hard").forEach { curve ->
                    FilterChip(
                        selected = pressureCurve == curve,
                        onClick = { pressureCurve = curve },
                        label = { Text(curve) },
                    )
                }
            }
            Spacer(Modifier.height(14.dp))
            MetricRow("Test pressure", "${(pressure * 100).toInt()}%")
            Slider(value = pressure, onValueChange = { pressure = it })
            LinearProgressIndicator(
                progress = { pressure.coerceIn(0f, 1f) },
                modifier = Modifier.fillMaxWidth(),
            )
            ToggleRow("Palm rejection hints", palmRejection) { palmRejection = it }
        }
        Spacer(Modifier.height(18.dp))

        InfoPanel {
            SectionTitle("Mapping")
            ChipRow {
                listOf("Fit", "Fill", "1:1", "Selected monitor").forEach { mode ->
                    FilterChip(
                        selected = mappingMode == mode,
                        onClick = { mappingMode = mode },
                        label = { Text(mode) },
                    )
                }
            }
            Spacer(Modifier.height(10.dp))
            MetricRow("Active mode", mappingMode)
            MetricRow("Calibration", calibrationStep.name)
        }
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
        StatusBand(
            items = listOf(
                StatusMetric("Resolution", settings.resolution.label, Tone.Primary),
                StatusMetric("Refresh", "${settings.maxFps} fps", Tone.Neutral),
                StatusMetric("Bitrate", "${settings.targetBitrateKbps / 1_000} Mbps", Tone.Neutral),
                StatusMetric("Codec", settings.codec.label, Tone.Primary),
            ),
        )
        Spacer(Modifier.height(16.dp))

        InfoPanel {
            SectionTitle("Stream profile")
            MetricRow("Current request", settings.summary)
            MetricRow("Display", selectedDisplayLabel(controlState))
            MetricRow("Color", settings.colorSpace.label)
            MetricRow("Keyframe interval", "${settings.keyframeIntervalMs} ms")
        }
        Spacer(Modifier.height(14.dp))

        if (controlState.displays.isNotEmpty()) {
            OptionGroup("Host display") {
                controlState.displays.forEach { display ->
                    FilterChip(
                        selected = settings.displayId == display.id,
                        onClick = { onSettingsChange(settings.copy(displayId = display.id)) },
                        label = { Text(display.label) },
                    )
                }
            }
            Spacer(Modifier.height(12.dp))
        }

        OptionGroup("Resolution") {
            ClientResolution.entries.forEach { resolution ->
                FilterChip(
                    selected = settings.resolution == resolution,
                    onClick = { onSettingsChange(settings.copy(resolution = resolution)) },
                    label = { Text(resolution.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        OptionGroup("Codec") {
            ClientVideoCodec.entries.forEach { codec ->
                FilterChip(
                    selected = settings.codec == codec,
                    onClick = { onSettingsChange(settings.copy(codec = codec)) },
                    label = { Text(codec.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        OptionGroup("Color space") {
            ClientColorSpace.entries.forEach { colorSpace ->
                FilterChip(
                    selected = settings.colorSpace == colorSpace,
                    onClick = { onSettingsChange(settings.copy(colorSpace = colorSpace)) },
                    label = { Text(colorSpace.label) },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        OptionGroup("Refresh rate") {
            listOf(60, 90, 120).forEach { fps ->
                FilterChip(
                    selected = settings.maxFps == fps,
                    onClick = { onSettingsChange(settings.copy(maxFps = fps)) },
                    label = { Text("$fps") },
                )
            }
        }
        Spacer(Modifier.height(12.dp))
        OptionGroup("Bitrate") {
            listOf(12_000, 18_000, 35_000, 60_000).forEach { bitrate ->
                FilterChip(
                    selected = settings.targetBitrateKbps == bitrate,
                    onClick = { onSettingsChange(settings.copy(targetBitrateKbps = bitrate)) },
                    label = { Text("${bitrate / 1_000} Mbps") },
                )
            }
        }
        Spacer(Modifier.height(18.dp))

        InfoPanel {
            SectionTitle("Client controls")
            ChipRow {
                ClientTouchMode.entries.forEach { touchMode ->
                    FilterChip(
                        selected = input.touchMode == touchMode,
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
}

private fun selectedDisplayLabel(controlState: SessionControlState): String {
    val displayId = controlState.videoSettings.displayId
    return controlState.displays
        .firstOrNull { it.id == displayId }
        ?.label
        ?: "Display $displayId"
}

@Composable
fun SecuritySettingsScreen() {
    ScreenFrame(
        title = "Security",
        subtitle = "Local trust, pairing, and device identity",
    ) {
        StatusBand(
            items = listOf(
                StatusMetric("Identity", "Keystore", Tone.Good),
                StatusMetric("Transport", "Encrypted", Tone.Good),
                StatusMetric("Clipboard", "Off", Tone.Neutral),
            ),
        )
        Spacer(Modifier.height(16.dp))
        InfoPanel {
            SectionTitle("Local trust")
            MetricRow("Device identity", "Android Keystore")
            MetricRow("Pairing", "One-time code / QR-ready")
            MetricRow("Session tokens", "Short-lived")
            MetricRow("Trusted hosts", "Per-device list")
        }
        Spacer(Modifier.height(12.dp))
        InfoPanel {
            SectionTitle("Privacy defaults")
            MetricRow("Keyboard logging", "Disabled")
            MetricRow("Clipboard sync", "Off")
            MetricRow("Stylus diagnostics", "Explicit screen only")
        }
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
        StatusBand(
            items = listOf(
                StatusMetric("Samples", state.totalSamples.toString(), Tone.Primary),
                StatusMetric("Hover", state.hoverSamples.toString(), Tone.Neutral),
                StatusMetric("Contact", state.contactSamples.toString(), Tone.Neutral),
                StatusMetric("Batch", state.lastBatchSize.toString(), Tone.Neutral),
            ),
        )
        Spacer(Modifier.height(14.dp))
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(min = 260.dp)
                .background(Color(0xFF070A0E), RoundedCornerShape(8.dp))
                .border(1.dp, MaterialTheme.colorScheme.primary.copy(alpha = 0.5f), RoundedCornerShape(8.dp))
                .pointerInteropFilter(onTouchEvent = controller::onMotionEvent),
            contentAlignment = Alignment.Center,
        ) {
            Text(latest?.toolType?.name ?: "Awaiting input", color = Color(0xFF9DB2BC))
        }
        Spacer(Modifier.height(16.dp))
        InfoPanel {
            SectionTitle("Raw sample")
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
        Spacer(Modifier.height(12.dp))
        InfoPanel {
            SectionTitle("Batch counters")
            MetricRow("Total samples", state.totalSamples.toString())
            MetricRow("Historical samples", state.historicalSamples.toString())
            MetricRow("Hover samples", state.hoverSamples.toString())
            MetricRow("Contact samples", state.contactSamples.toString())
            MetricRow("Last batch size", state.lastBatchSize.toString())
        }
    }
}

private enum class Tone {
    Primary,
    Good,
    Warning,
    Neutral,
}

private data class StatusMetric(
    val label: String,
    val value: String,
    val tone: Tone = Tone.Neutral,
)

@Composable
private fun SectionTitle(label: String) {
    Text(
        text = label,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onSurface,
    )
}

@Composable
private fun InfoPanel(
    modifier: Modifier = Modifier,
    content: @Composable ColumnScope.() -> Unit,
) {
    Surface(
        modifier = modifier
            .fillMaxWidth()
            .border(1.dp, MaterialTheme.colorScheme.outline.copy(alpha = 0.24f), RoundedCornerShape(8.dp)),
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.76f),
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(
            modifier = Modifier.padding(14.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
            content = content,
        )
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ChipRow(
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    FlowRow(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        content()
    }
}

@Composable
private fun StatusBand(items: List<StatusMetric>) {
    ChipRow {
        items.forEach { item ->
            StatusTile(item)
        }
    }
}

@Composable
private fun StatusTile(metric: StatusMetric) {
    val colors = toneColors(metric.tone)
    Surface(
        modifier = Modifier
            .widthIn(min = 148.dp)
            .heightIn(min = 64.dp),
        color = colors.first,
        shape = RoundedCornerShape(8.dp),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp),
            verticalArrangement = Arrangement.Center,
        ) {
            Text(metric.label, color = colors.second.copy(alpha = 0.72f), fontSize = 12.sp)
            Text(metric.value, color = colors.second, fontWeight = FontWeight.SemiBold, maxLines = 2)
        }
    }
}

@Composable
private fun StatusPill(label: String, tone: Tone = Tone.Neutral) {
    val colors = toneColors(tone)
    Surface(
        color = colors.first,
        shape = RoundedCornerShape(6.dp),
    ) {
        Text(
            text = label,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
            color = colors.second,
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
        )
    }
}

@Composable
private fun ReadinessRow(label: String, ready: Boolean) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant)
        StatusPill(if (ready) "Ready" else "Pending", if (ready) Tone.Good else Tone.Warning)
    }
}

@Composable
private fun OptionGroup(
    title: String,
    content: @Composable () -> Unit,
) {
    InfoPanel {
        SectionTitle(title)
        Spacer(Modifier.height(6.dp))
        ChipRow(content = content)
    }
}

@Composable
private fun toneColors(tone: Tone): Pair<Color, Color> {
    return when (tone) {
        Tone.Primary -> Color(0xFF11343E) to Color(0xFFB9F3FF)
        Tone.Good -> Color(0xFF14372E) to Color(0xFFB8F5D6)
        Tone.Warning -> Color(0xFF3B3114) to Color(0xFFFFE6A6)
        Tone.Neutral -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.74f) to MaterialTheme.colorScheme.onSurface
    }
}
