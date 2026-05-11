package com.glyphray.android.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Divider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.glyphray.android.input.StylusDiagnosticsController
import com.glyphray.android.ui.screens.ConnectionScreen
import com.glyphray.android.ui.screens.DiagnosticsScreen
import com.glyphray.android.ui.screens.HostListScreen
import com.glyphray.android.ui.screens.PairingScreen
import com.glyphray.android.ui.screens.PenSettingsScreen
import com.glyphray.android.ui.screens.RemoteSessionScreen
import com.glyphray.android.ui.screens.SecuritySettingsScreen
import com.glyphray.android.ui.screens.VideoSettingsScreen

enum class GlyphRayScreen(val label: String) {
    Hosts("Hosts"),
    Pair("Pair"),
    Connect("Connect"),
    Session("Session"),
    Pen("Pen"),
    Video("Video"),
    Security("Security"),
    Diagnostics("Diagnostics"),
}

@Composable
fun GlyphRayApp() {
    var screen by remember { mutableStateOf(GlyphRayScreen.Hosts) }
    val diagnosticsController = remember { StylusDiagnosticsController() }

    Scaffold(
        bottomBar = {
            NavigationBar(containerColor = MaterialTheme.colorScheme.surface) {
                listOf(
                    GlyphRayScreen.Hosts,
                    GlyphRayScreen.Session,
                    GlyphRayScreen.Pen,
                    GlyphRayScreen.Security,
                    GlyphRayScreen.Diagnostics,
                ).forEach { item ->
                    NavigationBarItem(
                        selected = screen == item,
                        onClick = { screen = item },
                        label = { Text(item.label) },
                        icon = { Text(item.label.first().toString()) },
                    )
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
                    onPair = { screen = GlyphRayScreen.Pair },
                    onConnect = { screen = GlyphRayScreen.Connect },
                )
                GlyphRayScreen.Pair -> PairingScreen(onDone = { screen = GlyphRayScreen.Hosts })
                GlyphRayScreen.Connect -> ConnectionScreen(onConnected = { screen = GlyphRayScreen.Session })
                GlyphRayScreen.Session -> RemoteSessionScreen(
                    onPenSettings = { screen = GlyphRayScreen.Pen },
                    onDiagnostics = { screen = GlyphRayScreen.Diagnostics },
                )
                GlyphRayScreen.Pen -> PenSettingsScreen(onDiagnostics = { screen = GlyphRayScreen.Diagnostics })
                GlyphRayScreen.Video -> VideoSettingsScreen()
                GlyphRayScreen.Security -> SecuritySettingsScreen()
                GlyphRayScreen.Diagnostics -> DiagnosticsScreen(diagnosticsController)
            }
        }
    }
}

@Composable
fun ScreenFrame(
    title: String,
    subtitle: String,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable Column.() -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .padding(20.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(title, style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.SemiBold)
                Text(subtitle, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), content = actions)
        }
        Spacer(Modifier.height(18.dp))
        Divider(color = MaterialTheme.colorScheme.outline.copy(alpha = 0.25f))
        Spacer(Modifier.height(18.dp))
        content()
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
        Text(value, fontWeight = FontWeight.Medium)
    }
}

@Composable
fun PrimaryAction(label: String, onClick: () -> Unit) {
    Button(
        onClick = onClick,
        colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
    ) {
        Text(label)
    }
}

@Composable
fun ToggleRow(label: String, checked: Boolean, onCheckedChange: (Boolean) -> Unit) {
    Row(
        modifier = Modifier.fillMaxWidth(),
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
