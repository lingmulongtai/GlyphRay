use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
use glyphray_protocol::{ColorSpace, DisplayDescriptor, EncoderConfig, VideoCodec};
use glyphray_transport::discovery::LanDiscoverySocket;
use glyphray_transport::udp::UdpServer;
use glyphray_transport::video::VideoPacketizer;
use glyphray_transport::TransportPacket;
use glyphray_windows_host::backend::{
    trusted_device_id, trusted_device_id_from_public_key_fingerprint, HostBackendRuntime,
    PermissionPolicy, PermissionState,
};
use glyphray_windows_host::capture::{ScreenCapture, WindowsGraphicsCaptureBackend};
use glyphray_windows_host::encoder::{EncoderSettings, PlatformVideoEncoder};
use glyphray_windows_host::input::{
    create_keyboard_injector, create_mouse_injector, create_pen_injector, create_touch_injector,
    KeyboardInjector, KeyboardInputBridge, MouseInjector, MouseInputBridge, PenInjector,
    StylusInputBridge, TouchInjector, TouchInputBridge,
};
use glyphray_windows_host::permission_ui::{
    permission_dialog_enabled, prompt_pairing_decision, PairingDecision, PairingPrompt,
};
use glyphray_windows_host::secrets::{load_or_create_host_identity, PlatformSecretStore};
use glyphray_windows_host::settings::{
    EncoderPreset, HostSettingsStore, TrustedDevice, TrustedDevicePermissions,
};
use glyphray_windows_host::startup::{StartupManager, StartupRegistration};
use glyphray_windows_host::streaming::VideoPacketPipeline;
use glyphray_windows_host::HostConfig;
use std::collections::HashSet;
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn Error>> {
    let config = HostConfig::default();
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("serve") => return run_backend(config),
        Some("startup") => return run_startup_command(args.next().as_deref()),
        Some(other) => {
            println!("Unknown command: {other}");
            print_top_level_help();
            return Ok(());
        }
        None => {}
    }

    println!("GlyphRay Windows Host");
    println!("Host name: {}", config.host_name);
    println!("Default display id: {}", config.default_display_id);
    println!("Control port: {}", config.control_port);
    println!("Discovery port: {}", config.discovery_port);
    print_top_level_help();

    match create_pen_injector() {
        Ok(_) => println!("Synthetic pen injector is available."),
        Err(err) => println!("Synthetic pen injector unavailable: {err}"),
    }
    Ok(())
}

fn print_top_level_help() {
    println!("Run `glyphray-windows-host serve` to start the backend runtime.");
    println!(
        "Run `glyphray-windows-host startup status|enable|disable` to manage user-logon startup."
    );
}

fn run_startup_command(action: Option<&str>) -> Result<(), Box<dyn Error>> {
    match action.unwrap_or("status") {
        "status" => print_startup_registration(StartupManager::status()?),
        "enable" => {
            let registration = StartupManager::enable()?;
            println!("User-logon startup enabled.");
            print_startup_registration(registration);
        }
        "disable" => {
            let registration = StartupManager::disable()?;
            println!("User-logon startup disabled.");
            print_startup_registration(registration);
        }
        _ => println!("Usage: glyphray-windows-host startup status|enable|disable"),
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
    let touch_bridge = create_runtime_touch_bridge(&config);
    let mouse_bridge = create_runtime_mouse_bridge(&config);
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
    let mut secret_store = PlatformSecretStore::open()?;
    let host_identity = load_or_create_host_identity(&mut secret_store)?;
    println!(
        "Host identity fingerprint: {}",
        host_identity.fingerprint()?
    );
    runtime.set_host_identity(host_identity);
    let settings_store = HostSettingsStore::open()?;
    let mut host_encoder_override = settings_store.load()?.encoder_override;
    if let Some(config) = host_encoder_override.as_ref() {
        println!(
            "Loaded saved encoder override: {}x{} {}fps {}kbps {:?} {:?}",
            config.width,
            config.height,
            config.max_fps,
            config.target_bitrate_kbps,
            config.codec,
            config.color_space
        );
    }
    let mut video_pump = create_runtime_video_pump(&runtime, host_encoder_override.as_ref());
    let (command_tx, commands) = mpsc::channel();
    spawn_console_command_reader(command_tx.clone());
    let mut permission_dialogs = PermissionDialogCoordinator::new(
        permission_policy == PermissionPolicy::RequireApproval && permission_dialog_enabled(),
        command_tx,
        runtime.config().host_name.clone(),
    );
    if permission_dialogs.enabled {
        println!("Native permission dialogs are enabled for pairing requests.");
    } else if permission_policy == PermissionPolicy::RequireApproval {
        println!(
            "Native permission dialogs are disabled. Set GLYPHRAY_ENABLE_PERMISSION_DIALOG=1 to approve pairing requests with a Windows dialog."
        );
    }
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
            let trusted_challenge = maybe_challenge_trusted_pairing(
                &event,
                &mut runtime,
                &mut server,
                &settings_store,
            )?;
            if trusted_challenge.is_none() {
                permission_dialogs.observe_backend_event(&event);
            }
            print_backend_event(&event);
            if let Some(events) = trusted_challenge {
                for event in events {
                    print_backend_event(&event);
                }
            }
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
            &settings_store,
        )? {
            print_backend_event(&event);
        }

        thread::sleep(Duration::from_millis(5));
    }
}

struct RuntimeVideoPump {
    pipeline: VideoPacketPipeline<WindowsGraphicsCaptureBackend, PlatformVideoEncoder>,
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
    if std::env::var_os("GLYPHRAY_DISABLE_VIDEO_STREAM").is_some() {
        println!("Video stream pump is disabled by GLYPHRAY_DISABLE_VIDEO_STREAM.");
        return None;
    }

    let capture = WindowsGraphicsCaptureBackend::new();
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
    let encoder = PlatformVideoEncoder::new(pump_settings.clone());
    let mut pump = VideoPacketPipeline::new(
        WindowsGraphicsCaptureBackend::new(),
        encoder,
        VideoPacketizer::default(),
        display.id,
    );
    match pump.start() {
        Ok(()) => {
            println!(
                "Video stream pump is enabled for display {} ({}x{}) at {}fps, {}kbps, {:?}, {:?}. Source={}. Encoder={:?}.",
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
                },
                pump_settings.backend
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
    if std::env::var_os("GLYPHRAY_DISABLE_KEYBOARD_INJECTION").is_some() {
        println!("Native keyboard injection is disabled by GLYPHRAY_DISABLE_KEYBOARD_INJECTION.");
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

fn create_runtime_touch_bridge(
    config: &HostConfig,
) -> Option<TouchInputBridge<Box<dyn TouchInjector>>> {
    if std::env::var_os("GLYPHRAY_DISABLE_TOUCH_INJECTION").is_some() {
        println!("Native touch injection is disabled by GLYPHRAY_DISABLE_TOUCH_INJECTION.");
        return None;
    }

    let injector = match create_touch_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Touch injector unavailable: {err}");
            return None;
        }
    };

    let mapper = mapper_for_default_display(config);
    println!("Native touch injection is enabled with default-display coordinate mapping.");
    Some(TouchInputBridge::new(injector, mapper))
}

fn create_runtime_mouse_bridge(
    config: &HostConfig,
) -> Option<MouseInputBridge<Box<dyn MouseInjector>>> {
    if std::env::var_os("GLYPHRAY_DISABLE_MOUSE_INJECTION").is_some() {
        println!("Native mouse injection is disabled by GLYPHRAY_DISABLE_MOUSE_INJECTION.");
        return None;
    }

    let injector = match create_mouse_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Mouse injector unavailable: {err}");
            return None;
        }
    };

    let mapper = mapper_for_default_display(config);
    println!("Native mouse injection is enabled with default-display coordinate mapping.");
    Some(MouseInputBridge::new(injector, mapper))
}

fn temporary_mapper() -> CoordinateMapper {
    let source = SourceRect::new(1920.0, 1080.0).expect("valid source rect");
    let display = DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).expect("valid display rect");
    CoordinateMapper::new(source, display, MappingMode::Stretch)
}

fn mapper_for_default_display(config: &HostConfig) -> CoordinateMapper {
    let capture = WindowsGraphicsCaptureBackend::new();
    let display = capture.list_displays().ok().and_then(|displays| {
        select_video_display(
            displays,
            config.default_display_id,
            config.default_display_id,
        )
    });

    let Some(display) = display else {
        println!(
            "Display-aware input mapper unavailable. Falling back to temporary 1920x1080 mapping."
        );
        return temporary_mapper();
    };

    println!(
        "Input mapper targets display {} {}x{} at origin {},{}.",
        display.id, display.width_px, display.height_px, display.origin_x, display.origin_y
    );
    let source = SourceRect::new(display.width_px as f32, display.height_px as f32)
        .unwrap_or_else(|_| SourceRect::new(1920.0, 1080.0).expect("fallback source rect"));
    let target = DisplayRect::new(
        display.origin_x as f32,
        display.origin_y as f32,
        display.width_px as f32,
        display.height_px as f32,
        display.rotation_degrees,
        display.scale_factor,
    )
    .unwrap_or_else(|_| {
        DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).expect("fallback display rect")
    });
    CoordinateMapper::new(source, target, MappingMode::Stretch)
}

fn create_runtime_input_bridge(
    config: &HostConfig,
) -> Option<StylusInputBridge<Box<dyn PenInjector>>> {
    if std::env::var_os("GLYPHRAY_DISABLE_PEN_INJECTION").is_some() {
        println!("Native pen injection is disabled by GLYPHRAY_DISABLE_PEN_INJECTION.");
        return None;
    }

    let injector = match create_pen_injector() {
        Ok(injector) => injector,
        Err(err) => {
            println!("Synthetic pen injector unavailable: {err}");
            return None;
        }
    };

    println!("Native pen injection is enabled with default-display coordinate mapping.");
    Some(StylusInputBridge::new(
        injector,
        mapper_for_default_display(config),
        PressureMapper::default(),
    ))
}

#[derive(Debug)]
enum HostCommand {
    Approve(SocketAddr),
    Reject(SocketAddr),
    PermissionDialogResult {
        peer: SocketAddr,
        decision: PairingDecision,
    },
    Status,
    Sessions,
    EncoderStatus,
    EncoderOverride(EncoderConfig),
    EncoderSave,
    EncoderPresetList,
    EncoderPresetSave(String),
    EncoderPresetApply(String),
    EncoderPresetDelete(String),
    EncoderClear,
    StartupStatus,
    StartupEnable,
    StartupDisable,
    TrustList,
    TrustForget(String),
    TrustPermission {
        id: String,
        input: TrustedInputKind,
        allow: bool,
    },
    TrustClear,
    Help,
}

#[derive(Debug, Clone, Copy)]
enum TrustedInputKind {
    Pen,
    Touch,
    Keyboard,
    Mouse,
    Gamepad,
}

struct PermissionDialogCoordinator {
    enabled: bool,
    tx: Sender<HostCommand>,
    host_name: String,
    active_prompts: HashSet<SocketAddr>,
}

impl PermissionDialogCoordinator {
    fn new(enabled: bool, tx: Sender<HostCommand>, host_name: String) -> Self {
        Self {
            enabled,
            tx,
            host_name,
            active_prompts: HashSet::new(),
        }
    }

    fn observe_backend_event(&mut self, event: &glyphray_windows_host::backend::BackendEvent) {
        use glyphray_windows_host::backend::BackendEvent;
        match event {
            BackendEvent::PairingRequested {
                peer, device_name, ..
            } => {
                if !self.enabled || !self.active_prompts.insert(*peer) {
                    return;
                }

                let prompt = PairingPrompt {
                    device_name: device_name.clone(),
                    peer: *peer,
                    host_name: self.host_name.clone(),
                };
                let tx = self.tx.clone();
                thread::spawn(move || {
                    let decision = prompt_pairing_decision(&prompt);
                    let _ = tx.send(HostCommand::PermissionDialogResult {
                        peer: prompt.peer,
                        decision,
                    });
                });
            }
            BackendEvent::PeerApproved { peer } | BackendEvent::PeerRejected { peer } => {
                self.active_prompts.remove(peer);
            }
            _ => {}
        }
    }
}

fn spawn_console_command_reader(tx: Sender<HostCommand>) {
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
        "startup" => parse_startup_command(parts.collect()),
        "trust" => parse_trust_command(parts.collect()),
        "help" => Some(HostCommand::Help),
        _ => None,
    }
}

fn parse_trust_command(parts: Vec<&str>) -> Option<HostCommand> {
    match parts.as_slice() {
        [] | ["list"] => Some(HostCommand::TrustList),
        ["forget", id] | ["remove", id] => Some(HostCommand::TrustForget((*id).to_string())),
        ["permission", id, input, value] => Some(HostCommand::TrustPermission {
            id: (*id).to_string(),
            input: parse_trusted_input_kind(input)?,
            allow: parse_permission_value(value)?,
        }),
        ["clear"] => Some(HostCommand::TrustClear),
        _ => None,
    }
}

fn parse_trusted_input_kind(value: &str) -> Option<TrustedInputKind> {
    match value {
        "pen" => Some(TrustedInputKind::Pen),
        "touch" => Some(TrustedInputKind::Touch),
        "keyboard" => Some(TrustedInputKind::Keyboard),
        "mouse" => Some(TrustedInputKind::Mouse),
        "gamepad" | "controller" => Some(TrustedInputKind::Gamepad),
        _ => None,
    }
}

fn parse_permission_value(value: &str) -> Option<bool> {
    match value {
        "on" | "allow" | "true" | "1" => Some(true),
        "off" | "deny" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_startup_command(parts: Vec<&str>) -> Option<HostCommand> {
    match parts.as_slice() {
        [] | ["status"] => Some(HostCommand::StartupStatus),
        ["enable"] => Some(HostCommand::StartupEnable),
        ["disable"] => Some(HostCommand::StartupDisable),
        _ => None,
    }
}

fn parse_encoder_command(parts: Vec<&str>) -> Option<HostCommand> {
    match parts.as_slice() {
        [] | ["status"] => Some(HostCommand::EncoderStatus),
        ["save"] => Some(HostCommand::EncoderSave),
        ["preset"] | ["preset", "list"] => Some(HostCommand::EncoderPresetList),
        ["preset", "save", name] => Some(HostCommand::EncoderPresetSave((*name).to_string())),
        ["preset", "apply", name] => Some(HostCommand::EncoderPresetApply((*name).to_string())),
        ["preset", "delete", name] | ["preset", "remove", name] => {
            Some(HostCommand::EncoderPresetDelete((*name).to_string()))
        }
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
    settings_store: &HostSettingsStore,
) -> Result<Vec<glyphray_windows_host::backend::BackendEvent>, Box<dyn Error>> {
    let mut events = Vec::new();
    while let Ok(command) = commands.try_recv() {
        match command {
            HostCommand::Approve(peer) => {
                events.extend(approve_peer_and_record_trust(
                    runtime,
                    server,
                    settings_store,
                    peer,
                )?);
            }
            HostCommand::Reject(peer) => {
                events.extend(runtime.reject_peer_and_notify(
                    server,
                    peer,
                    "Rejected by host operator",
                )?);
            }
            HostCommand::PermissionDialogResult { peer, decision } => {
                if !peer_is_pending(runtime, peer) {
                    println!(
                        "Ignoring stale permission dialog result for {peer}; the session is no longer pending."
                    );
                    continue;
                }
                match decision {
                    PairingDecision::Approve => {
                        events.extend(approve_peer_and_record_trust(
                            runtime,
                            server,
                            settings_store,
                            peer,
                        )?);
                    }
                    PairingDecision::Reject => {
                        events.extend(runtime.reject_peer_and_notify(
                            server,
                            peer,
                            "Rejected by host permission dialog",
                        )?);
                    }
                }
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
                let saved_settings = settings_store.load().ok();
                let saved_override = saved_settings
                    .as_ref()
                    .and_then(|settings| settings.encoder_override.as_ref());
                let saved_presets = saved_settings
                    .as_ref()
                    .map(|settings| settings.encoder_presets.as_slice())
                    .unwrap_or(&[]);
                print_encoder_status(
                    host_encoder_override.as_ref(),
                    saved_override,
                    saved_presets,
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
            HostCommand::EncoderSave => {
                let config = host_encoder_override
                    .clone()
                    .or_else(|| runtime.latest_approved_encoder_config());
                let Some(config) = config else {
                    println!(
                        "No host override or approved client EncoderConfig is available to save."
                    );
                    continue;
                };

                match settings_store.save_encoder_override(config.clone()) {
                    Ok(_) => {
                        println!(
                            "Saved encoder override: {}x{} {}fps {}kbps {:?} {:?}",
                            config.width,
                            config.height,
                            config.max_fps,
                            config.target_bitrate_kbps,
                            config.codec,
                            config.color_space
                        );
                        *host_encoder_override = Some(config);
                        *video_pump =
                            create_runtime_video_pump(runtime, host_encoder_override.as_ref());
                        *last_video_frame = Instant::now();
                    }
                    Err(error) => println!("Saving encoder override failed: {error}"),
                }
            }
            HostCommand::EncoderPresetList => match settings_store.load() {
                Ok(settings) => print_encoder_presets(&settings.encoder_presets),
                Err(error) => println!("Loading encoder presets failed: {error}"),
            },
            HostCommand::EncoderPresetSave(name) => {
                let config = host_encoder_override
                    .clone()
                    .or_else(|| runtime.latest_approved_encoder_config());
                let Some(config) = config else {
                    println!(
                        "No host override or approved client EncoderConfig is available to save as a preset."
                    );
                    continue;
                };

                match settings_store.save_encoder_preset(&name, config.clone()) {
                    Ok(_) => println!(
                        "Saved encoder preset `{name}`: {}x{} {}fps {}kbps {:?} {:?}",
                        config.width,
                        config.height,
                        config.max_fps,
                        config.target_bitrate_kbps,
                        config.codec,
                        config.color_space
                    ),
                    Err(error) => println!("Saving encoder preset `{name}` failed: {error}"),
                }
            }
            HostCommand::EncoderPresetApply(name) => {
                match settings_store.load_encoder_preset(&name) {
                    Ok(Some(config)) => {
                        println!(
                            "Applied encoder preset `{name}`: {}x{} {}fps {}kbps {:?} {:?}",
                            config.width,
                            config.height,
                            config.max_fps,
                            config.target_bitrate_kbps,
                            config.codec,
                            config.color_space
                        );
                        *host_encoder_override = Some(config);
                        *video_pump =
                            create_runtime_video_pump(runtime, host_encoder_override.as_ref());
                        *last_video_frame = Instant::now();
                    }
                    Ok(None) => println!("Encoder preset `{name}` was not found."),
                    Err(error) => println!("Loading encoder preset `{name}` failed: {error}"),
                }
            }
            HostCommand::EncoderPresetDelete(name) => {
                match settings_store.delete_encoder_preset(&name) {
                    Ok((_, true)) => println!("Deleted encoder preset `{name}`."),
                    Ok((_, false)) => println!("Encoder preset `{name}` was not found."),
                    Err(error) => println!("Deleting encoder preset `{name}` failed: {error}"),
                }
            }
            HostCommand::EncoderClear => {
                println!("Host encoder override cleared. Approved client EncoderConfig will be used when available.");
                *host_encoder_override = None;
                if let Err(error) = settings_store.clear_encoder_override() {
                    println!("Clearing saved encoder override failed: {error}");
                }
                *video_pump = create_runtime_video_pump(runtime, host_encoder_override.as_ref());
                *last_video_frame = Instant::now();
            }
            HostCommand::StartupStatus => match StartupManager::status() {
                Ok(registration) => print_startup_registration(registration),
                Err(error) => println!("startup status failed: {error}"),
            },
            HostCommand::StartupEnable => match StartupManager::enable() {
                Ok(registration) => {
                    println!("User-logon startup enabled.");
                    print_startup_registration(registration);
                }
                Err(error) => println!("startup enable failed: {error}"),
            },
            HostCommand::StartupDisable => match StartupManager::disable() {
                Ok(registration) => {
                    println!("User-logon startup disabled.");
                    print_startup_registration(registration);
                }
                Err(error) => println!("startup disable failed: {error}"),
            },
            HostCommand::TrustList => match settings_store.load() {
                Ok(settings) => print_trusted_devices(&settings.trusted_devices),
                Err(error) => println!("Loading trusted devices failed: {error}"),
            },
            HostCommand::TrustForget(id) => match settings_store.forget_trusted_device(&id) {
                Ok((_, true)) => println!("Forgot trusted device `{id}`."),
                Ok((_, false)) => println!("Trusted device `{id}` was not found."),
                Err(error) => println!("Forgetting trusted device `{id}` failed: {error}"),
            },
            HostCommand::TrustPermission { id, input, allow } => {
                match update_trusted_device_permission(settings_store, &id, input, allow) {
                    Ok(Some(permissions)) => {
                        let active = runtime.update_device_permissions(&id, permissions);
                        println!(
                            "Updated trusted device `{id}` permission {input:?}={allow}; active sessions updated={active}."
                        );
                    }
                    Ok(None) => println!("Trusted device `{id}` was not found."),
                    Err(error) => println!("Updating trusted device `{id}` failed: {error}"),
                }
            }
            HostCommand::TrustClear => match settings_store.clear_trusted_devices() {
                Ok(_) => println!("All trusted devices were removed."),
                Err(error) => println!("Clearing trusted devices failed: {error}"),
            },
            HostCommand::Help => {
                println!("Commands:");
                println!("  status");
                println!("  sessions");
                println!("  trust list");
                println!("  trust forget <trusted-device-id>");
                println!("  trust permission <trusted-device-id> <pen|touch|keyboard|mouse|gamepad> <on|off>");
                println!("  trust clear");
                println!("  encoder status");
                println!("  encoder override <width>x<height> <fps> <kbps>");
                println!("  encoder save");
                println!("  encoder preset list");
                println!("  encoder preset save <name>");
                println!("  encoder preset apply <name>");
                println!("  encoder preset delete <name>");
                println!("  encoder clear");
                println!("  startup status");
                println!("  startup enable");
                println!("  startup disable");
                println!("  approve <ip:port>");
                println!("  reject <ip:port>");
            }
        }
    }
    Ok(events)
}

fn maybe_challenge_trusted_pairing(
    event: &glyphray_windows_host::backend::BackendEvent,
    runtime: &mut HostBackendRuntime<Box<dyn PenInjector>>,
    server: &mut UdpServer,
    settings_store: &HostSettingsStore,
) -> Result<Option<Vec<glyphray_windows_host::backend::BackendEvent>>, Box<dyn Error>> {
    let glyphray_windows_host::backend::BackendEvent::PairingRequested {
        peer,
        public_key_fingerprint: Some(fingerprint),
        ..
    } = event
    else {
        return Ok(None);
    };

    let settings = settings_store.load()?;
    let Some(device) = settings.trusted_devices.iter().find(|device| {
        device.public_key_fingerprint.as_deref() == Some(fingerprint.as_str())
            && device.public_key_der.is_some()
    }) else {
        return Ok(None);
    };

    println!(
        "Trusted device matched by public key fingerprint: id={} label=`{}` peer={peer}",
        device.id, device.label
    );
    let events = runtime.challenge_peer_as_and_notify_with_permissions(
        server,
        *peer,
        device.id.clone(),
        device.permissions.clone(),
    )?;
    Ok(Some(events))
}

fn update_trusted_device_permission(
    settings_store: &HostSettingsStore,
    id: &str,
    input: TrustedInputKind,
    allow: bool,
) -> Result<Option<TrustedDevicePermissions>, Box<dyn Error>> {
    let settings = settings_store.load()?;
    let Some(mut device) = settings
        .trusted_devices
        .into_iter()
        .find(|device| device.id.eq_ignore_ascii_case(id))
    else {
        return Ok(None);
    };
    match input {
        TrustedInputKind::Pen => device.permissions.allow_pen = allow,
        TrustedInputKind::Touch => device.permissions.allow_touch = allow,
        TrustedInputKind::Keyboard => device.permissions.allow_keyboard = allow,
        TrustedInputKind::Mouse => device.permissions.allow_mouse = allow,
        TrustedInputKind::Gamepad => device.permissions.allow_gamepad = allow,
    }
    let permissions = device.permissions.clone();
    settings_store.upsert_trusted_device(device)?;
    Ok(Some(permissions))
}

fn approve_peer_and_record_trust(
    runtime: &mut HostBackendRuntime<Box<dyn PenInjector>>,
    server: &mut UdpServer,
    settings_store: &HostSettingsStore,
    peer: SocketAddr,
) -> Result<Vec<glyphray_windows_host::backend::BackendEvent>, Box<dyn Error>> {
    let label = pending_device_label(runtime, peer).unwrap_or_else(|| peer.to_string());
    let fingerprint = pending_public_key_fingerprint(runtime, peer);
    let public_key_der = pending_public_key_der(runtime, peer);
    let id = fingerprint
        .as_deref()
        .map(trusted_device_id_from_public_key_fingerprint)
        .unwrap_or_else(|| trusted_device_id(peer));
    let events = runtime.approve_peer_as_and_notify(server, peer, id.clone())?;
    match TrustedDevice::approved_now(
        id.clone(),
        label.clone(),
        peer.to_string(),
        fingerprint,
        public_key_der,
    )
    .and_then(|device| settings_store.upsert_trusted_device(device))
    {
        Ok(_) => println!("Trusted device recorded: id={id} label={label} peer={peer}"),
        Err(error) => println!("Recording trusted device `{id}` failed: {error}"),
    }
    Ok(events)
}

fn pending_device_label(
    runtime: &HostBackendRuntime<Box<dyn PenInjector>>,
    peer: SocketAddr,
) -> Option<String> {
    runtime
        .session_snapshots()
        .into_iter()
        .find(|session| session.peer == peer)
        .and_then(|session| session.device_id)
}

fn pending_public_key_fingerprint(
    runtime: &HostBackendRuntime<Box<dyn PenInjector>>,
    peer: SocketAddr,
) -> Option<String> {
    runtime
        .session_snapshots()
        .into_iter()
        .find(|session| session.peer == peer)
        .and_then(|session| session.device_public_key_fingerprint)
}

fn pending_public_key_der(
    runtime: &HostBackendRuntime<Box<dyn PenInjector>>,
    peer: SocketAddr,
) -> Option<Vec<u8>> {
    runtime
        .session_snapshots()
        .into_iter()
        .find(|session| session.peer == peer)
        .and_then(|session| session.device_public_key_der)
}

fn peer_is_pending(runtime: &HostBackendRuntime<Box<dyn PenInjector>>, peer: SocketAddr) -> bool {
    runtime
        .session_snapshots()
        .iter()
        .any(|session| session.peer == peer && session.permission == PermissionState::Pending)
}

fn print_trusted_devices(devices: &[TrustedDevice]) {
    if devices.is_empty() {
        println!("trusted devices=none");
        return;
    }

    println!("trusted devices={}", devices.len());
    for device in devices {
        println!(
            "trusted id={} label=`{}` peer={} public_key={} public_key_der={} bytes approved_unix_ms={} permissions=pen:{},touch:{},keyboard:{},mouse:{},gamepad:{}",
            device.id,
            device.label,
            device.last_peer,
            device.public_key_fingerprint.as_deref().unwrap_or("-"),
            device.public_key_der.as_ref().map(Vec::len).unwrap_or(0),
            device.approved_unix_ms,
            device.permissions.allow_pen,
            device.permissions.allow_touch,
            device.permissions.allow_keyboard,
            device.permissions.allow_mouse,
            device.permissions.allow_gamepad
        );
    }
}

fn print_startup_registration(registration: StartupRegistration) {
    println!("startup enabled={}", registration.enabled);
    if let Some(command) = registration.command {
        println!("startup command={command}");
    }
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
    saved_override: Option<&EncoderConfig>,
    saved_presets: &[EncoderPreset],
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

    match saved_override {
        Some(config) => println!(
            "encoder saved={}x{} {}fps {}kbps {:?} {:?}",
            config.width,
            config.height,
            config.max_fps,
            config.target_bitrate_kbps,
            config.codec,
            config.color_space
        ),
        None => println!("encoder saved=none"),
    }

    print_encoder_presets(saved_presets);

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

fn print_encoder_presets(presets: &[EncoderPreset]) {
    if presets.is_empty() {
        println!("encoder presets=none");
        return;
    }

    println!("encoder presets={}", presets.len());
    for preset in presets {
        println!(
            "encoder preset `{}`={}x{} {}fps {}kbps {:?} {:?}",
            preset.name,
            preset.config.width,
            preset.config.height,
            preset.config.max_fps,
            preset.config.target_bitrate_kbps,
            preset.config.codec,
            preset.config.color_space
        );
    }
}

fn print_backend_event(event: &glyphray_windows_host::backend::BackendEvent) {
    use glyphray_windows_host::backend::BackendEvent;
    match event {
        BackendEvent::PairingRequested {
            peer,
            device_name,
            public_key_fingerprint,
        } => {
            println!("Pairing requested from {device_name} at {peer}.");
            if let Some(fingerprint) = public_key_fingerprint {
                println!("Device public key fingerprint: {fingerprint}");
            }
            println!("Type `approve {peer}` to trust it or `reject {peer}` to deny it.");
        }
        BackendEvent::PairingResultQueued { peer, accepted } => {
            println!("PairingResult queued for {peer}: accepted={accepted}");
        }
        BackendEvent::AuthChallengeQueued { peer, challenge_id } => {
            println!(
                "Trusted-device auth challenge queued for {peer}: challenge_id={challenge_id}"
            );
        }
        BackendEvent::TrustedDeviceAuthenticated {
            peer,
            trusted_device_id,
        } => {
            println!("Trusted device authenticated: peer={peer} id={trusted_device_id}");
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
    fn parses_trusted_device_permission_command() {
        assert!(matches!(
            parse_host_command("trust permission trusted-key-abc keyboard off"),
            Some(HostCommand::TrustPermission {
                id,
                input: TrustedInputKind::Keyboard,
                allow: false,
            }) if id == "trusted-key-abc"
        ));
    }

    #[test]
    fn parses_encoder_save_command() {
        assert!(matches!(
            parse_host_command("encoder save"),
            Some(HostCommand::EncoderSave)
        ));
    }

    #[test]
    fn parses_encoder_preset_commands() {
        assert!(matches!(
            parse_host_command("encoder preset list"),
            Some(HostCommand::EncoderPresetList)
        ));
        assert!(matches!(
            parse_host_command("encoder preset save studio-120"),
            Some(HostCommand::EncoderPresetSave(name)) if name == "studio-120"
        ));
        assert!(matches!(
            parse_host_command("encoder preset apply studio-120"),
            Some(HostCommand::EncoderPresetApply(name)) if name == "studio-120"
        ));
        assert!(matches!(
            parse_host_command("encoder preset delete studio-120"),
            Some(HostCommand::EncoderPresetDelete(name)) if name == "studio-120"
        ));
    }

    #[test]
    fn parses_startup_commands() {
        assert!(matches!(
            parse_host_command("startup status"),
            Some(HostCommand::StartupStatus)
        ));
        assert!(matches!(
            parse_host_command("startup enable"),
            Some(HostCommand::StartupEnable)
        ));
        assert!(matches!(
            parse_host_command("startup disable"),
            Some(HostCommand::StartupDisable)
        ));
    }

    #[test]
    fn parses_trust_commands() {
        assert!(matches!(
            parse_host_command("trust list"),
            Some(HostCommand::TrustList)
        ));
        assert!(matches!(
            parse_host_command("trust forget trusted-tablet"),
            Some(HostCommand::TrustForget(id)) if id == "trusted-tablet"
        ));
        assert!(matches!(
            parse_host_command("trust clear"),
            Some(HostCommand::TrustClear)
        ));
    }

    #[test]
    fn permission_dialog_coordinator_is_noop_when_disabled() {
        let (tx, rx) = mpsc::channel();
        let mut coordinator =
            PermissionDialogCoordinator::new(false, tx, "GlyphRay Host".to_string());
        let peer: SocketAddr = "127.0.0.1:44999".parse().expect("peer");
        coordinator.observe_backend_event(
            &glyphray_windows_host::backend::BackendEvent::PairingRequested {
                peer,
                device_name: "Tablet".to_string(),
                public_key_fingerprint: None,
            },
        );

        assert!(coordinator.active_prompts.is_empty());
        assert!(rx.try_recv().is_err());
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
