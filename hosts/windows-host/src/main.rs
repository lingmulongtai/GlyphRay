use glyphray_transport::discovery::LanDiscoverySocket;
use glyphray_transport::udp::UdpServer;
use glyphray_windows_host::backend::{HostBackendRuntime, NoopPenInjector};
use glyphray_windows_host::{input::create_pen_injector, HostConfig};
use std::error::Error;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
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
    let mut runtime = HostBackendRuntime::<NoopPenInjector>::new(config, None);
    let mut last_announce = Instant::now() - Duration::from_secs(2);

    println!("GlyphRay backend listening on {}", server.local_addr()?);
    loop {
        if last_announce.elapsed() >= Duration::from_secs(1) {
            discovery.announce(runtime.advertisement())?;
            last_announce = Instant::now();
        }

        for event in runtime.poll_control(&mut server)? {
            println!("backend event: {event:?}");
        }

        thread::sleep(Duration::from_millis(5));
    }
}
