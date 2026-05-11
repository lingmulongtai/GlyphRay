use glyphray_telemetry::LatencyBreakdown;
use glyphray_transport::ConnectionStats;

#[derive(Debug, Clone, PartialEq)]
pub struct HostDiagnosticsSnapshot {
    pub connected_clients: usize,
    pub selected_display_id: u32,
    pub encoder_name: String,
    pub target_bitrate_kbps: u32,
    pub connection: ConnectionStats,
    pub latency: LatencyBreakdown,
    pub pen_injection_available: bool,
}

impl HostDiagnosticsSnapshot {
    pub fn health_label(&self) -> &'static str {
        if self.connected_clients == 0 {
            "idle"
        } else if self.connection.packet_loss_percent >= 8.0 {
            "unstable"
        } else if self.latency.video_total_us() > 35_000 {
            "latent"
        } else {
            "healthy"
        }
    }
}

