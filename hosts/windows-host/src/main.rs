use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
use glyphray_transport::discovery::LanDiscoverySocket;
use glyphray_transport::udp::UdpServer;
use glyphray_windows_host::backend::{HostBackendRuntime, PermissionPolicy};
use glyphray_windows_host::input::{create_pen_injector, PenInjector, StylusInputBridge};
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
    let permission_policy = if std::env::var_os("GLYPHRAY_DEV_AUTO_APPROVE").is_some() {
        println!("Development auto-approval is enabled for incoming LAN clients.");
        PermissionPolicy::DevAutoApprove
    } else {
        PermissionPolicy::RequireApproval
    };
    let mut runtime = HostBackendRuntime::<Box<dyn PenInjector>>::new_with_permission_policy(
        config,
        input_bridge,
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

    let source = SourceRect::new(1920.0, 1080.0).expect("valid source rect");
    let display = DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).expect("valid display rect");
    let mapper = CoordinateMapper::new(source, display, MappingMode::Stretch);

    println!("Native pen injection is enabled with temporary 1920x1080 stretch mapping.");
    Some(StylusInputBridge::new(
        injector,
        mapper,
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
                            "session peer={} device={} permission={:?} packets={}",
                            session.peer,
                            session.device_id.as_deref().unwrap_or("-"),
                            session.permission,
                            session.packets_received
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
        BackendEvent::PeerApproved { peer } => println!("Peer approved: {peer}"),
        BackendEvent::PeerRejected { peer } => println!("Peer rejected: {peer}"),
        other => println!("backend event: {other:?}"),
    }
}
