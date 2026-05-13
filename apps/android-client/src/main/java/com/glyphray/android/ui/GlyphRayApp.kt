package com.glyphray.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Brush
import androidx.compose.material.icons.rounded.CastConnected
import androidx.compose.material.icons.rounded.Devices
import androidx.compose.material.icons.rounded.HealthAndSafety
import androidx.compose.material.icons.rounded.QueryStats
import androidx.compose.material.icons.rounded.VideoSettings
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.glyphray.android.input.StylusDiagnosticsController
import com.glyphray.android.network.DiscoveredHost
import com.glyphray.android.network.HostDiscoveryController
import com.glyphray.android.network.SessionControlController
import com.glyphray.android.ui.screens.ConnectionScreen
import com.glyphray.android.ui.screens.DiagnosticsScreen
import com.glyphray.android.ui.screens.HostListScreen
import com.glyphray.android.ui.screens.PairingScreen
import com.glyphray.android.ui.screens.PenSettingsScreen
import com.glyphray.android.ui.screens.RemoteSessionScreen
import com.glyphray.android.ui.screens.SecuritySettingsScreen
import com.glyphray.android.ui.screens.VideoSettingsScreen

enum class GlyphRayScreen(val label: String, val icon: ImageVector) {
    Hosts("Hosts", Icons.Rounded.Devices),
    Pair("Pair", Icons.Rounded.CastConnected),
    Connect("Connect", Icons.Rounded.CastConnected),
    Session("Session", Icons.Rounded.CastConnected),
    Pen("Pen", Icons.Rounded.Brush),
    Video("Video", Icons.Rounded.VideoSettings),
    Security("Security", Icons.Rounded.HealthAndSafety),
    Diagnostics("Diagnostics", Icons.Rounded.QueryStats),
}

@Composable
fun GlyphRayApp() {
    var screen by remember { mutableStateOf(GlyphRayScreen.Hosts) }
    var selectedHost by remember { mutableStateOf<DiscoveredHost?>(null) }
    var sessionFullscreen by remember { mutableStateOf(false) }
    val diagnosticsController = remember { StylusDiagnosticsController() }
    val hostDiscoveryController = remember { HostDiscoveryController() }
    val sessionControlController = remember { SessionControlController() }

    DisposableEffect(hostDiscoveryController) {
        hostDiscoveryController.startContinuousScan()
        onDispose { hostDiscoveryController.close() }
    }

    DisposableEffect(sessionControlController) {
        onDispose { sessionControlController.close() }
    }

    Scaffold(
        bottomBar = {
            if (!(screen == GlyphRayScreen.Session && sessionFullscreen)) {
                NavigationBar(containerColor = MaterialTheme.colorScheme.surface) {
                    listOf(
                        GlyphRayScreen.Hosts,
                        GlyphRayScreen.Session,
                        GlyphRayScreen.Pen,
                        GlyphRayScreen.Video,
                        GlyphRayScreen.Security,
                        GlyphRayScreen.Diagnostics,
                    ).forEach { item ->
                        NavigationBarItem(
                            selected = screen == item,
                            onClick = { screen = item },
                            label = { Text(item.label) },
                            icon = { Icon(item.icon, contentDescription = item.label) },
                        )
                    }
                }
            }
        },
    ) { padding ->
        Surface(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
            color = MaterialTheme.colorScheme.background,
        ) {
            when (screen) {
                GlyphRayScreen.Hosts -> HostListScreen(
                    discoveryState = hostDiscoveryController.state,
                    onRefresh = hostDiscoveryController::refreshOnce,
                    onAddManualHost = hostDiscoveryController::addManualHost,
                    onPair = { screen = GlyphRayScreen.Pair },
                    onConnect = { host ->
                        selectedHost = host
                        screen = GlyphRayScreen.Connect
                    },
                )
                GlyphRayScreen.Pair -> PairingScreen(onDone = { screen = GlyphRayScreen.Hosts })
                GlyphRayScreen.Connect -> ConnectionScreen(
                    selectedHost = selectedHost,
                    controlState = sessionControlController.state,
                    onConnected = {
                        selectedHost?.let { host ->
                            sessionControlController.connect(host)
                            sessionControlController.sendPairingRequest()
                            sessionControlController.sendLatencyPing()
                        }
                        screen = GlyphRayScreen.Session
                    },
                )
                GlyphRayScreen.Session -> RemoteSessionScreen(
                    selectedHost = selectedHost,
                    controlState = sessionControlController.state,
                    fullscreen = sessionFullscreen,
                    onFullscreenChange = { fullscreen ->
                        sessionFullscreen = fullscreen
                        sessionControlController.updateInputSettings(
                            sessionControlController.state.inputSettings.copy(fullscreenMode = fullscreen),
                        )
                    },
                    onVideoSettings = { screen = GlyphRayScreen.Video },
                    onSpecialKey = sessionControlController::sendSpecialKey,
                    onKeyEvent = sessionControlController::onKeyEvent,
                    onGenericMotionEvent = sessionControlController::onGenericMotionEvent,
                    onVideoStreamController = sessionControlController::attachVideoStreamController,
                    onPenSettings = { screen = GlyphRayScreen.Pen },
                    onDiagnostics = { screen = GlyphRayScreen.Diagnostics },
                )
                GlyphRayScreen.Pen -> PenSettingsScreen(onDiagnostics = { screen = GlyphRayScreen.Diagnostics })
                GlyphRayScreen.Video -> VideoSettingsScreen(
                    controlState = sessionControlController.state,
                    onSettingsChange = sessionControlController::updateVideoSettings,
                    onInputSettingsChange = sessionControlController::updateInputSettings,
                    onApply = sessionControlController::sendEncoderConfig,
                )
                GlyphRayScreen.Security -> SecuritySettingsScreen()
                GlyphRayScreen.Diagnostics -> DiagnosticsScreen(diagnosticsController)
            }
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ScreenFrame(
    title: String,
    subtitle: String,
    actions: @Composable () -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background),
    ) {
        Column(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .fillMaxWidth()
                .widthIn(max = 1120.dp)
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(14.dp),
                verticalAlignment = Alignment.Top,
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(title, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
                    Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    actions()
                }
            }
            Spacer(Modifier.height(18.dp))
            HorizontalDivider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f))
            Spacer(Modifier.height(18.dp))
            content()
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
fun MetricRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 7.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Spacer(Modifier.width(16.dp))
        Text(
            value,
            modifier = Modifier.weight(1f),
            textAlign = TextAlign.End,
            fontWeight = FontWeight.Medium,
        )
    }
}

@Composable
fun PrimaryAction(label: String, onClick: () -> Unit) {
    PrimaryAction(label = label, enabled = true, onClick = onClick)
}

@Composable
fun PrimaryAction(label: String, enabled: Boolean, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        enabled = enabled,
        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
    ) {
        Text(label)
    }
}

@Composable
fun ToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label)
        Switch(checked = checked, onCheckedChange = onCheckedChange)
    }
}

@Composable
fun PlaceholderRemoteSurface(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .height(320.dp)
            .background(Color(0xFF0A0D11)),
        contentAlignment = Alignment.Center,
    ) {
        Text("No video stream", color = Color(0xFF8FA3AD))
    }
}
