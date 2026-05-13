use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
use glyphray_protocol::{ColorSpace, DisplayDescriptor, EncoderConfig, VideoCodec};
use glyphray_transport::discovery::LanDiscoverySocket;
use glyphray_transport::udp::UdpServer;
use glyphray_transport::video::VideoPacketizer;
use glyphray_transport::TransportPacket;
use glyphray_windows_host::backend::{HostBackendRuntime, PermissionPolicy};
use glyphray_windows_host::capture::{ScreenCapture, WindowsGraphicsCaptureBackend};
use glyphray_windows_host::encoder::{EncoderSettings, PendingHardwareEncoder};
use glyphray_windows_host::input::{
    create_keyboard_injector, create_mouse_injector, create_pen_injector, create_touch_injector,
    KeyboardInjector, KeyboardInputBridge, MouseInjector, MouseInputBridge, PenInjector,
    StylusInputBridge, TouchInjector, TouchInputBridge,
};
use glyphray_windows_host::streaming::VideoPacketPipeline;
use glyphray_windows_host::HostConfig;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let config = HostConfig::default();
    if std::env::args().any(|arg| arg == "serve") {
        return run_backend(config);
    }

    println!("GlyphRay Windows Host");
    println!("Host name: {}", config.host_name);
    println!("Default display id: {}", config.default_display_id);
    println!("Control port: {}", config.control_port);
    println!("Discovery port: {}", config.discovery_port);
    println!("Run `glyphray-windows-host serve` to start the backend runtime.");

    match create_pen_injector() {
        Ok(_) => println!("Synthetic pen injector is available."),
        Err(err) => println!("Synthetic pen injector unavailable: {err}"),
    }
    Ok(())
}

fn run_backend(config: HostConfig) -> Result<(), Box<dyn Error>> {
    let control_addr = SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        config.control_port,
    ));
    let mut server = UdpServer::bind(control_addr)?;
    let discovery = LanDiscoverySocket::bind(config.discovery_port)?;
    let input_bridge = create_runtime_input_bridge(&config);
    let keyboard_bridge = create_runtime_keyboard_bridge();
    let touch_bridge = create_runtime_touch_bridge();
    let mouse_bridge = create_runtime_mouse_bridge();
    let permission_policy = if std::env::var_os("GLYPHRAY_DEV_AUTO_APPROVE").is_some() {
        println!("Development auto-approval is enabled for incoming LAN clients.");
        PermissionPolicy::DevAutoApprove
    } else {
        PermissionPolicy::RequireApproval
    };
    let mut runtime = HostBackendRuntime::<Box<dyn PenInjector>>::new_with_input_bridges(
        config,
        input_bridge,
        keyboard_bridge,
        touch_bridge,
        mouse_bridge,
        permission_policy,
    );
    let mut host_encoder_override: Option<EncoderConfig> = None;
    let mut video_pump = create_runtime_video_pump(&runtime, host_encoder_override.as_ref());
    let commands = spawn_console_command_reader();
    let mut last_announce = Instant::now() - Duration::from_secs(2);
    let mut last_video_frame = Instant::now();

    println!("GlyphRay backend listening on {}", server.local_addr()?);
    println!("Type `status`, `sessions`, `approve <peer>`, `reject <peer>`, or `help`.");
    loop {
        if last_announce.elapsed() >= Duration::from_secs(1) {
            discovery.announce(runtime.advertisement())?;
            last_announce = Instant::now();
        }

        let mut should_restart_video_pump = false;
        for event in runtime.poll_control(&mut server)? {
            if matches!(
                event,
                glyphray_windows_host::backend::BackendEvent::EncoderConfigUpdated { .. }
            ) {
                should_restart_video_pump = true;
            }
            print_backend_event(&event);
        }
        if should_restart_video_pump && host_encoder_override.is_none() {
            video_pump = create_runtime_video_pump(&runtime, host_encoder_override.as_ref());
            last_video_frame = Instant::now();
        }

        if let Some(pump) = video_pump.as_mut() {
            if last_video_frame.elapsed() >= pump.frame_interval {
                match pump.capture_encode_packetize() {
                    Ok(packets) => {
                        for event in runtime.queue_video_packets_for_approved_peers(packets) {
                            print_backend_event(&event);
                        }
                        for event in runtime.flush_outbound(&mut server)? {
                            print_backend_event(&event);
                        }
                    }
                    Err(error) => {
                        println!("Video pump failed: {error}");
                        video_pump = None;
                    }
                }
                last_video_frame = Instant::now();
            }
        }

        for event in drain_console_commands(
            &commands,
            &mut runtime,
            &mut server,
            &mut host_encoder_override,
            &mut video_pump,
            &mut last_video_frame,
        )? {
            print_backend_event(&event);
        }

        thread::sleep(Duration::from_millis(5));
    }
}

struct RuntimeVideoPump {
    pipeline: VideoPacketPipeline<WindowsGraphicsCaptureBackend, PendingHardwareEncoder>,
    settings: EncoderSettings,
    display_id: u32,
    frame_interval: Duration,
    source: &'static str,
}

impl RuntimeVideoPump {
    fn capture_encode_packetize(
        &mut self,
    ) -> Result<Vec<TransportPacket>, glyphray_windows_host::streaming::StreamError> {
        self.pipeline.capture_encode_packetize()
    }
}

fn create_runtime_video_pump(
    runtime: &HostBackendRuntime<Box<dyn PenInjector>>,
    host_override: Option<&EncoderConfig>,
) -> Option<RuntimeVideoPump> {
    if std::env::var_os("GLYPHRAY_ENABLE_VIDEO_STREAM").is_none() {
        println!(
            "Video stream pump is disabled. Set GLYPHRAY_ENABLE_VIDEO_STREAM=1 to queue H.264 video fragments for approved clients."
        );
        return None;
    }

    let capture = WindowsGraphicsCaptureBackend;
    let client_config = runtime.latest_approved_encoder_config();
    let requested_config = host_override.or(client_config.as_ref());
    let requested_display_id = requested_config
        .map(|config| config.display_id)
        .unwrap_or_else(|| runtime.config().default_display_id);
    let display = match capture.list_displays() {
        Ok(displays) => select_video_display(
            displays,
            requested_display_id,
            runtime.config().default_display_id,
        ),
        Err(error) => {
            println!("Video stream unavailable: {error}");
            None
        }
    }?;

    let pump_settings = encoder_settings_for_display(&display, requested_config);
    let frame_interval = frame_interval_for_fps(pump_settings.fps);
    let encoder = PendingHardwareEncoder::new(pump_settings.clone());
    let mut pump = VideoPacketPipeline::new(
        WindowsGraphicsCaptureBackend,
        encoder,
        VideoPacketizer::default(),
        display.id,
    );
    match pump.start() {
        Ok(()) => {
            println!(
                "Video stream pump is enabled for display {} ({}x{}) at {}fps, {}kbps, {:?}, {:?}. Source={}. Encoder backend is still the placeholder abstraction until a concrete H.264 backend lands.",
                display.id,
                display.width_px,
                display.height_px,
                pump_settings.fps,
                pump_settings.target_bitrate_kbps,
                pump_settings.codec,
                pump_settings.color_space,
                if host_override.is_some() {
                    "host override"
                } else if requested_config.is_some() {
                    "client config"
                } else {
                    "default"
                }
            );
            Some(RuntimeVideoPump {
                pipeline: pump,
                settings: pump_settings,
                display_id: display.id,
                frame_interval,
                source: if host_override.is_some() {
                    "host override"
                } else if requested_config.is_some() {
                    "client config"
                } else {
                    "default"
                },
            })
        }
        Err(error) => {
            println!("Video stream unavailable: {error}");
            None
        }
    }
}

fn select_video_display(
    displays: Vec<DisplayDescriptor>,
    requested_display_id: u32,
    fallback_display_id: u32,
) -> Option<DisplayDescriptor> {
    displays
        .iter()
        .find(|display| display.id == requested_display_id)
        .cloned()
        .or_else(|| {
            if requested_display_id != fallback_display_id {
                println!(
                    "Requested display {requested_display_id} was not found. Falling back to display {fallback_display_id}."
                );
            }
            displays
                .iter()
                .find(|display| display.id == fallback_display_id)
                .cloned()
        })
        .or_else(|| displays.into_iter().next())
}

fn encoder_settings_for_display(
    display: &DisplayDescriptor,
    requested: Option<&EncoderConfig>,
) -> EncoderSettings {
    let Some(config) = requested else {
        return EncoderSettings::low_latency_h264(display.width_px, display.height_px, 60);
    };

    let mut settings = EncoderSettings::low_latency_h264(
        display.width_px,
        display.height_px,
        config.max_fps.clamp(30, 120),
    );
    settings.codec = config.codec;
    settings.color_space = config.color_space;
    settings.target_bitrate_kbps = config.target_bitrate_kbps.clamp(4_000, 120_000);
    settings.keyframe_interval_ms = config.keyframe_interval_ms.clamp(250, 10_000);
    settings.allow_b_frames = !config.low_latency;

    if config.width == display.width_px && config.height == display.height_px {
        settings.width = config.width;
        settings.height = config.height;
    } else {
        println!(
            "Requested encoder resolution {}x{} differs from captured display {}x{}. Using display-native capture until scaler support lands.",
            config.width, config.height, display.width_px, display.height_px
        );
    }

    settings
}

fn frame_interval_for_fps(fps: u16) -> Duration {
    Duration::from_micros(1_000_000_u64 / u64::from(fps.max(1)))
}

fn create_runtime_keyboard_bridge() -> Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>> {
    if std::env::var_os("GLYPHRAY_ENABLE_KEYBOARD_INJECTION").is_none() {
        println!(
            "Native keyboard injection is disabled. Set GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1 for LAN keyboard smoke tests."
        );
        return None;
    }

    let injector = match create_keyboard_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Keyboard injector unavailable: {err}");
            return None;
        }
    };

    println!("Native keyboard injection is enabled for approved clients.");
    Some(KeyboardInputBridge::new(injector))
}

fn create_runtime_touch_bridge() -> Option<TouchInputBridge<Box<dyn TouchInjector>>> {
    if std::env::var_os("GLYPHRAY_ENABLE_TOUCH_INJECTION").is_none() {
        println!(
            "Native touch injection is disabled. Set GLYPHRAY_ENABLE_TOUCH_INJECTION=1 for LAN touch smoke tests."
        );
        return None;
    }

    let injector = match create_touch_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Touch injector unavailable: {err}");
            return None;
        }
    };

    println!("Native touch injection is enabled with temporary 1920x1080 stretch mapping.");
    Some(TouchInputBridge::new(injector, temporary_mapper()))
}

fn create_runtime_mouse_bridge() -> Option<MouseInputBridge<Box<dyn MouseInjector>>> {
    if std::env::var_os("GLYPHRAY_ENABLE_MOUSE_INJECTION").is_none() {
        println!(
            "Native mouse injection is disabled. Set GLYPHRAY_ENABLE_MOUSE_INJECTION=1 for LAN mouse smoke tests."
        );
        return None;
    }

    let injector = match create_mouse_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Mouse injector unavailable: {err}");
            return None;
        }
    };

    println!("Native mouse injection is enabled with temporary 1920x1080 stretch mapping.");
    Some(MouseInputBridge::new(injector, temporary_mapper()))
}

fn temporary_mapper() -> CoordinateMapper {
    let source = SourceRect::new(1920.0, 1080.0).expect("valid source rect");
    let display = DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).expect("valid display rect");
    CoordinateMapper::new(source, display, MappingMode::Stretch)
}

fn create_runtime_input_bridge(
    _config: &HostConfig,
) -> Option<StylusInputBridge<Box<dyn PenInjector>>> {
    if std::env::var_os("GLYPHRAY_ENABLE_PEN_INJECTION").is_none() {
        println!(
            "Native pen injection is disabled. Set GLYPHRAY_ENABLE_PEN_INJECTION=1 for LAN input smoke tests."
        );
        return None;
    }

    let injector = match create_pen_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Synthetic pen injector unavailable: {err}");
            return None;
        }
    };

    println!("Native pen injection is enabled with temporary 1920x1080 stretch mapping.");
    Some(StylusInputBridge::new(
        injector,
        temporary_mapper(),
        PressureMapper::default(),
    ))
}

#[derive(Debug)]
enum HostCommand {
    Approve(SocketAddr),
    Reject(SocketAddr),
    Status,
    Sessions,
    EncoderStatus,
    EncoderOverride(EncoderConfig),
    EncoderClear,
    Help,
}

fn spawn_console_command_reader() -> Receiver<HostCommand> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            if stdin.read_line(&mut line).is_err() {
                continue;
            }
            let Some(command) = parse_host_command(&line) else {
                println!("Unrecognized command. Type `help`.");
                continue;
            };
            if tx.send(command).is_err() {
                break;
            }
        }
    });
    rx
}

fn parse_host_command(line: &str) -> Option<HostCommand> {
    let mut parts = line.split_whitespace();
    match parts.next()? {
        "approve" => parts
            .next()
            .and_then(|peer| peer.parse().ok())
            .map(HostCommand::Approve),
        "reject" => parts
            .next()
            .and_then(|peer| peer.parse().ok())
            .map(HostCommand::Reject),
        "status" => Some(HostCommand::Status),
        "sessions" => Some(HostCommand::Sessions),
        "encoder" => parse_encoder_command(parts.collect()),
        "help" => Some(HostCommand::Help),
        _ => None,
    }
}

fn parse_encoder_command(parts: Vec<&str>) -> Option<HostCommand> {
    match parts.as_slice() {
        [] | ["status"] => Some(HostCommand::EncoderStatus),
        ["clear"] => Some(HostCommand::EncoderClear),
        ["override", size, fps, bitrate] => {
            let (width, height) = parse_size(size)?;
            Some(HostCommand::EncoderOverride(EncoderConfig {
                display_id: 0,
                codec: VideoCodec::H264,
                color_space: ColorSpace::Rec709,
                width,
                height,
                max_fps: fps.parse::<u16>().ok()?.clamp(30, 120),
                target_bitrate_kbps: bitrate.parse::<u32>().ok()?.clamp(4_000, 120_000),
                keyframe_interval_ms: 1_000,
                low_latency: true,
            }))
        }
        _ => None,
    }
}

fn parse_size(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.split_once('x').or_else(|| value.split_once('X'))?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn drain_console_commands(
    commands: &Receiver<HostCommand>,
    runtime: &mut HostBackendRuntime<Box<dyn PenInjector>>,
    server: &mut UdpServer,
    host_encoder_override: &mut Option<EncoderConfig>,
    video_pump: &mut Option<RuntimeVideoPump>,
    last_video_frame: &mut Instant,
) -> Result<Vec<glyphray_windows_host::backend::BackendEvent>, Box<dyn Error>> {
    let mut events = Vec::new();
    while let Ok(command) = commands.try_recv() {
        match command {
            HostCommand::Approve(peer) => {
                events.extend(runtime.approve_peer_and_notify(server, peer)?);
            }
            HostCommand::Reject(peer) => {
                events.extend(runtime.reject_peer_and_notify(
                    server,
                    peer,
                    "Rejected by host operator",
                )?);
            }
            HostCommand::Status => {
                print_backend_status(runtime.health_snapshot());
            }
            HostCommand::Sessions => {
                let sessions = runtime.session_snapshots();
                if sessions.is_empty() {
                    println!("No client sessions yet.");
                } else {
                    for session in sessions {
                        println!(
                            "session peer={} device={} permission={:?} packets={} encoder={}",
                            session.peer,
                            session.device_id.as_deref().unwrap_or("-"),
                            session.permission,
                            session.packets_received,
                            session
                                .encoder_config
                                .as_ref()
                                .map(|config| format!(
                                    "{}x{} {}fps {}kbps {:?} {:?}",
                                    config.width,
                                    config.height,
                                    config.max_fps,
                                    config.target_bitrate_kbps,
                                    config.codec,
                                    config.color_space
                                ))
                                .unwrap_or_else(|| "-".to_string())
                        );
                    }
                }
            }
            HostCommand::EncoderStatus => {
                print_encoder_status(
                    host_encoder_override.as_ref(),
                    runtime.latest_approved_encoder_config().as_ref(),
                    video_pump.as_ref(),
                );
            }
            HostCommand::EncoderOverride(config) => {
                println!(
                    "Host encoder override set: {}x{} {}fps {}kbps {:?} {:?}",
                    config.width,
                    config.height,
                    config.max_fps,
                    config.target_bitrate_kbps,
                    config.codec,
                    config.color_space
                );
                *host_encoder_override = Some(config);
                *video_pump = create_runtime_video_pump(runtime, host_encoder_override.as_ref());
                *last_video_frame = Instant::now();
            }
            HostCommand::EncoderClear => {
                println!("Host encoder override cleared. Approved client EncoderConfig will be used when available.");
                *host_encoder_override = None;
                *video_pump = create_runtime_video_pump(runtime, host_encoder_override.as_ref());
                *last_video_frame = Instant::now();
            }
            HostCommand::Help => {
                println!("Commands:");
                println!("  status");
                println!("  sessions");
                println!("  encoder status");
                println!("  encoder override <width>x<height> <fps> <kbps>");
                println!("  encoder clear");
                println!("  approve <ip:port>");
                println!("  reject <ip:port>");
            }
        }
    }
    Ok(events)
}

fn print_backend_status(snapshot: glyphray_windows_host::backend::BackendHealthSnapshot) {
    println!(
        "status sessions={} pending={} outbound_total={} input={} control={} audio={} video={}",
        snapshot.sessions_total,
        snapshot.pending_sessions,
        snapshot.outbound.total,
        snapshot.outbound.input,
        snapshot.outbound.control,
        snapshot.outbound.audio,
        snapshot.outbound.video,
    );
    println!(
        "status metrics received={} queued={} sent={} backpressure={} dropped_outbound={} late_input_drops={} pending_rate_limited={}",
        snapshot.metrics.received_packets,
        snapshot.metrics.queued_outbound_packets,
        snapshot.metrics.sent_outbound_packets,
        snapshot.metrics.backpressure_events,
        snapshot.outbound.dropped_packets_total,
        snapshot.metrics.late_input_dropped_packets,
        snapshot.metrics.pending_rate_limited_packets,
    );
    println!(
        "status queue_high_watermark={} capacity_per_channel={}",
        snapshot.outbound.high_watermark, snapshot.outbound.capacity_per_channel,
    );
}

fn print_encoder_status(
    host_override: Option<&EncoderConfig>,
    client_config: Option<&EncoderConfig>,
    pump: Option<&RuntimeVideoPump>,
) {
    match host_override {
        Some(config) => println!(
            "encoder override={}x{} {}fps {}kbps {:?} {:?}",
            config.width,
            config.height,
            config.max_fps,
            config.target_bitrate_kbps,
            config.codec,
            config.color_space
        ),
        None => println!("encoder override=none"),
    }

    match client_config {
        Some(config) => println!(
            "encoder client={}x{} {}fps {}kbps {:?} {:?}",
            config.width,
            config.height,
            config.max_fps,
            config.target_bitrate_kbps,
            config.codec,
            config.color_space
        ),
        None => println!("encoder client=none"),
    }

    match pump {
        Some(pump) => println!(
            "encoder pump=active display={} source={} effective={}x{} {}fps {}kbps {:?} {:?}",
            pump.display_id,
            pump.source,
            pump.settings.width,
            pump.settings.height,
            pump.settings.fps,
            pump.settings.target_bitrate_kbps,
            pump.settings.codec,
            pump.settings.color_space
        ),
        None => println!("encoder pump=inactive"),
    }
}

fn print_backend_event(event: &glyphray_windows_host::backend::BackendEvent) {
    use glyphray_windows_host::backend::BackendEvent;
    match event {
        BackendEvent::PairingRequested { peer, device_name } => {
            println!("Pairing requested from {device_name} at {peer}.");
            println!("Type `approve {peer}` to trust it or `reject {peer}` to deny it.");
        }
        BackendEvent::PairingResultQueued { peer, accepted } => {
            println!("PairingResult queued for {peer}: accepted={accepted}");
        }
        BackendEvent::DisplayInfoQueued { peer, displays } => {
            println!("DisplayInfo queued for {peer}: {displays} display(s)");
        }
        BackendEvent::PeerApproved { peer } => println!("Peer approved: {peer}"),
        BackendEvent::PeerRejected { peer } => println!("Peer rejected: {peer}"),
        BackendEvent::EncoderConfigUpdated {
            peer,
            width,
            height,
            max_fps,
            target_bitrate_kbps,
        } => {
            println!("Encoder config from {peer}: {width}x{height} {max_fps}fps {target_bitrate_kbps}kbps");
        }
        BackendEvent::KeyboardDecoded {
            peer,
            virtual_key,
            pressed,
        } => {
            println!("Keyboard input from {peer}: vk={virtual_key} pressed={pressed}");
        }
        BackendEvent::KeyboardInjected {
            peer,
            virtual_key,
            pressed,
        } => {
            println!("Keyboard injected for {peer}: vk={virtual_key} pressed={pressed}");
        }
        BackendEvent::TouchDecoded { peer, samples } => {
            println!("Touch input from {peer}: {samples} sample(s)");
        }
        BackendEvent::TouchInjected { peer, samples } => {
            println!("Touch injected for {peer}: {samples} sample(s)");
        }
        BackendEvent::MouseDecoded { peer, button_flags } => {
            println!("Mouse input from {peer}: buttons={button_flags}");
        }
        BackendEvent::MouseInjected {
            peer,
            injected_events,
        } => {
            println!("Mouse injected for {peer}: {injected_events} event(s)");
        }
        BackendEvent::GamepadDecoded {
            peer,
            controller_id,
            buttons,
        } => {
            println!("Gamepad input from {peer}: controller={controller_id} buttons={buttons}");
        }
        other => println!("backend event: {other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_override_command() {
        let command = parse_host_command("encoder override 1920x1080 120 35000").expect("command");
        let HostCommand::EncoderOverride(config) = command else {
            panic!("expected encoder override");
        };
        assert_eq!(config.width, 1920);
        assert_eq!(config.height, 1080);
        assert_eq!(config.max_fps, 120);
        assert_eq!(config.target_bitrate_kbps, 35_000);
        assert_eq!(config.codec, VideoCodec::H264);
        assert_eq!(config.color_space, ColorSpace::Rec709);
    }

    #[test]
    fn encoder_settings_keep_capture_native_until_scaling_lands() {
        let display = DisplayDescriptor {
            id: 0,
            name: "Primary".to_string(),
            origin_x: 0,
            origin_y: 0,
            width_px: 2560,
            height_px: 1440,
            scale_factor: 1.0,
            rotation_degrees: 0,
            refresh_hz: 120.0,
            primary: true,
        };
        let request = EncoderConfig {
            display_id: 0,
            codec: VideoCodec::H264,
            color_space: ColorSpace::DisplayP3,
            width: 1920,
            height: 1080,
            max_fps: 120,
            target_bitrate_kbps: 35_000,
            keyframe_interval_ms: 500,
            low_latency: true,
        };

        let settings = encoder_settings_for_display(&display, Some(&request));

        assert_eq!(settings.width, 2560);
        assert_eq!(settings.height, 1440);
        assert_eq!(settings.fps, 120);
        assert_eq!(settings.target_bitrate_kbps, 35_000);
        assert_eq!(settings.color_space, ColorSpace::DisplayP3);
        assert!(!settings.allow_b_frames);
    }
}
