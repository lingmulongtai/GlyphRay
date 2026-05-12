use glyphray_telemetry::LatencyBreakdown;
use glyphray_transport::ConnectionStats;
use glyphray_windows_host::diagnostics::HostDiagnosticsSnapshot;

fn main() {
    let snapshot = HostDiagnosticsSnapshot {
        connected_clients: 0,
        selected_display_id: 0,
        encoder_name: "H.264 low-latency".to_string(),
        target_bitrate_kbps: 18_000,
        connection: ConnectionStats {
            rtt_ms: 0.0,
            jitter_ms: 0.0,
            packet_loss_percent: 0.0,
            estimated_bandwidth_kbps: 0,
        },
        latency: LatencyBreakdown::default(),
        pen_injection_available: cfg!(windows),
    };

    println!("GlyphRay Host Diagnostics");
    println!("Health: {}", snapshot.health_label());
    println!("Connected clients: {}", snapshot.connected_clients);
    println!("Selected display: {}", snapshot.selected_display_id);
    println!("Encoder: {}", snapshot.encoder_name);
    println!("Target bitrate: {} kbps", snapshot.target_bitrate_kbps);
    println!(
        "Pen injection available: {}",
        snapshot.pen_injection_available
    );
}
