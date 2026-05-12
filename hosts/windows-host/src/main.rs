use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
use glyphray_transport::discovery::LanDiscoverySocket;
use glyphray_transport::udp::UdpServer;
use glyphray_windows_host::backend::{HostBackendRuntime, PermissionPolicy};
use glyphray_windows_host::input::{
    create_keyboard_injector, create_mouse_injector, create_pen_injector, create_touch_injector,
    KeyboardInjector, KeyboardInputBridge, MouseInjector, MouseInputBridge, PenInjector,
    StylusInputBridge, TouchInjector, TouchInputBridge,
};
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
    let commands = spawn_console_command_reader();
    let mut last_announce = Instant::now() - Duration::from_secs(2);

    println!("GlyphRay backend listening on {}", server.local_addr()?);
    println!("Type `sessions`, `approve <peer>`, `reject <peer>`, or `help`.");
    loop {
        if last_announce.elapsed() >= Duration::from_secs(1) {
            discovery.announce(runtime.advertisement())?;
            last_announce = Instant::now();
        }

        for event in runtime.poll_control(&mut server)? {
            print_backend_event(&event);
        }

        for event in drain_console_commands(&commands, &mut runtime, &mut server)? {
            print_backend_event(&event);
        }

        thread::sleep(Duration::from_millis(5));
    }
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
    Sessions,
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
        "sessions" => Some(HostCommand::Sessions),
        "help" => Some(HostCommand::Help),
        _ => None,
    }
}

fn drain_console_commands(
    commands: &Receiver<HostCommand>,
    runtime: &mut HostBackendRuntime<Box<dyn PenInjector>>,
    server: &mut UdpServer,
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
            HostCommand::Help => {
                println!("Commands:");
                println!("  sessions");
                println!("  approve <ip:port>");
                println!("  reject <ip:port>");
            }
        }
    }
    Ok(events)
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
