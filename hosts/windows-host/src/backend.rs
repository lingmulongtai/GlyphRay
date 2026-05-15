use crate::capture::{ScreenCapture, WindowsGraphicsCaptureBackend};
use crate::config::HostConfig;
use crate::input::{
    InjectionReport, InputError, KeyboardInjector, KeyboardInputBridge, MouseInjector,
    MouseInputBridge, PenInjector, StylusInputBridge, TouchInjector, TouchInputBridge,
};
use glyphray_protocol::stylus_wire::{decode_stylus_batch, StylusWireError};
use glyphray_protocol::{
    decode_frame, encode_frame, DisplayDescriptor, DisplayInfo, EncoderConfig, GamepadInput,
    KeyboardInput, LatencyPing, LatencyPong, Message, MessageKind, MouseInput, PairingResult,
    TouchInputBatch,
};
use glyphray_transport::discovery::HostAdvertisement;
use glyphray_transport::udp::UdpServer;
use glyphray_transport::{ChannelKind, TransportError, TransportPacket};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_PENDING_SESSIONS: usize = 50;
const MAX_PENDING_ATTEMPTS_PER_IP: usize = 12;
const PENDING_ATTEMPT_WINDOW: Duration = Duration::from_secs(10);
const MAX_OUTBOUND_QUEUE_PER_CHANNEL: usize = 128;
const OUTBOUND_FLUSH_BUDGET: usize = 8;
const OUTBOUND_QOS_SCHEDULE: [ChannelKind; 8] = [
    ChannelKind::Input,
    ChannelKind::Control,
    ChannelKind::Input,
    ChannelKind::Audio,
    ChannelKind::Control,
    ChannelKind::Video,
    ChannelKind::Input,
    ChannelKind::Control,
];
const LATE_INPUT_PACKET_REASON: &str = "late input packet";

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    StylusWire(#[from] StylusWireError),
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("protocol payload was not understood: {0}")]
    Protocol(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionPolicy {
    RequireApproval,
    DevAutoApprove,
}

#[derive(Debug, Clone)]
pub struct ClientSession {
    pub peer: SocketAddr,
    pub device_id: Option<String>,
    pub device_public_key_fingerprint: Option<String>,
    pub permission: PermissionState,
    pub packets_received: u64,
    pub last_seen: Instant,
    pub encoder_config: Option<EncoderConfig>,
    pub last_input_sequence: Option<u64>,
    pub last_input_timestamp_us: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub peer: SocketAddr,
    pub device_id: Option<String>,
    pub device_public_key_fingerprint: Option<String>,
    pub permission: PermissionState,
    pub packets_received: u64,
    pub encoder_config: Option<EncoderConfig>,
    pub last_input_sequence: Option<u64>,
    pub last_input_timestamp_us: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendMetrics {
    pub received_packets: u64,
    pub queued_outbound_packets: u64,
    pub queued_video_packets: u64,
    pub sent_outbound_packets: u64,
    pub backpressure_events: u64,
    pub pending_rate_limited_packets: u64,
    pub late_input_dropped_packets: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutboundQueueSnapshot {
    pub input: usize,
    pub control: usize,
    pub audio: usize,
    pub video: usize,
    pub total: usize,
    pub capacity_per_channel: usize,
    pub dropped_packets_total: u64,
    pub high_watermark: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendHealthSnapshot {
    pub sessions_total: usize,
    pub pending_sessions: usize,
    pub outbound: OutboundQueueSnapshot,
    pub metrics: BackendMetrics,
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<SocketAddr, ClientSession>,
    pending_attempts_by_ip: HashMap<IpAddr, VecDeque<Instant>>,
}

impl SessionRegistry {
    pub fn contains_peer(&self, peer: SocketAddr) -> bool {
        self.sessions.contains_key(&peer)
    }

    pub fn allow_new_pending_peer(&mut self, peer: SocketAddr, now: Instant) -> bool {
        let attempts = self.pending_attempts_by_ip.entry(peer.ip()).or_default();
        while attempts
            .front()
            .is_some_and(|attempt| now.duration_since(*attempt) > PENDING_ATTEMPT_WINDOW)
        {
            attempts.pop_front();
        }
        if attempts.len() >= MAX_PENDING_ATTEMPTS_PER_IP {
            return false;
        }
        attempts.push_back(now);
        true
    }

    pub fn ensure_pending(&mut self, peer: SocketAddr) -> &mut ClientSession {
        if !self.sessions.contains_key(&peer) {
            self.evict_oldest_pending_if_needed();
        }

        self.sessions.entry(peer).or_insert_with(|| ClientSession {
            peer,
            device_id: None,
            device_public_key_fingerprint: None,
            permission: PermissionState::Pending,
            packets_received: 0,
            last_seen: Instant::now(),
            encoder_config: None,
            last_input_sequence: None,
            last_input_timestamp_us: None,
        })
    }

    pub fn approve(&mut self, peer: SocketAddr, device_id: impl Into<String>) {
        let session = self.ensure_pending(peer);
        session.permission = PermissionState::Approved;
        session.device_id = Some(device_id.into());
    }

    pub fn reject(&mut self, peer: SocketAddr) {
        self.ensure_pending(peer).permission = PermissionState::Rejected;
    }

    pub fn is_approved(&self, peer: SocketAddr) -> bool {
        self.sessions
            .get(&peer)
            .map(|session| session.permission == PermissionState::Approved)
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|session| session.permission == PermissionState::Pending)
            .count()
    }

    pub fn accept_input_watermark(
        &mut self,
        peer: SocketAddr,
        sequence: u64,
        timestamp_us: u64,
    ) -> bool {
        let Some(session) = self.sessions.get_mut(&peer) else {
            return false;
        };

        if session
            .last_input_sequence
            .is_some_and(|last| sequence <= last)
            || session
                .last_input_timestamp_us
                .is_some_and(|last| timestamp_us < last)
        {
            return false;
        }

        session.last_input_sequence = Some(sequence);
        session.last_input_timestamp_us = Some(timestamp_us);
        true
    }

    pub fn snapshots(&self) -> Vec<SessionSnapshot> {
        let mut snapshots = self
            .sessions
            .values()
            .map(|session| SessionSnapshot {
                peer: session.peer,
                device_id: session.device_id.clone(),
                device_public_key_fingerprint: session.device_public_key_fingerprint.clone(),
                permission: session.permission,
                packets_received: session.packets_received,
                encoder_config: session.encoder_config.clone(),
                last_input_sequence: session.last_input_sequence,
                last_input_timestamp_us: session.last_input_timestamp_us,
            })
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|session| session.peer);
        snapshots
    }

    pub fn approved_peers(&self) -> Vec<SocketAddr> {
        let mut peers = self
            .sessions
            .values()
            .filter(|session| session.permission == PermissionState::Approved)
            .map(|session| session.peer)
            .collect::<Vec<_>>();
        peers.sort();
        peers
    }

    fn evict_oldest_pending_if_needed(&mut self) {
        if self.pending_count() < MAX_PENDING_SESSIONS {
            return;
        }

        let oldest = self
            .sessions
            .iter()
            .filter(|(_, session)| session.permission == PermissionState::Pending)
            .min_by_key(|(_, session)| session.last_seen)
            .map(|(peer, _)| *peer);
        if let Some(peer) = oldest {
            self.sessions.remove(&peer);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendEvent {
    SessionDiscovered {
        peer: SocketAddr,
    },
    PeerAutoApproved {
        peer: SocketAddr,
    },
    PeerApproved {
        peer: SocketAddr,
    },
    PeerRejected {
        peer: SocketAddr,
    },
    PairingRequested {
        peer: SocketAddr,
        device_name: String,
        public_key_fingerprint: Option<String>,
    },
    PairingResultQueued {
        peer: SocketAddr,
        accepted: bool,
    },
    DisplayInfoQueued {
        peer: SocketAddr,
        displays: usize,
    },
    EncoderConfigUpdated {
        peer: SocketAddr,
        width: u32,
        height: u32,
        max_fps: u16,
        target_bitrate_kbps: u32,
    },
    KeyboardDecoded {
        peer: SocketAddr,
        virtual_key: u32,
        pressed: bool,
    },
    KeyboardInjected {
        peer: SocketAddr,
        virtual_key: u32,
        pressed: bool,
    },
    TouchDecoded {
        peer: SocketAddr,
        samples: usize,
    },
    TouchInjected {
        peer: SocketAddr,
        samples: usize,
    },
    MouseDecoded {
        peer: SocketAddr,
        button_flags: u32,
    },
    MouseInjected {
        peer: SocketAddr,
        injected_events: usize,
    },
    GamepadDecoded {
        peer: SocketAddr,
        controller_id: u32,
        buttons: u32,
    },
    PermissionRequired {
        peer: SocketAddr,
    },
    PacketIgnored {
        peer: SocketAddr,
        reason: String,
    },
    PendingRateLimited {
        peer: SocketAddr,
    },
    StylusInjected {
        peer: SocketAddr,
        samples: usize,
    },
    StylusDecoded {
        peer: SocketAddr,
        samples: usize,
    },
    LatencyPongQueued {
        peer: SocketAddr,
    },
    OutboundQueued {
        peer: SocketAddr,
        packets: usize,
    },
    VideoFrameQueued {
        peers: usize,
        packets: usize,
    },
    OutboundBackpressure {
        peer: SocketAddr,
        queued_packets: usize,
    },
    PacketRouted {
        peer: SocketAddr,
        channel: ChannelKind,
    },
}

#[derive(Debug, Default)]
pub struct RouteOutcome {
    pub events: Vec<BackendEvent>,
    pub outbound: Vec<(SocketAddr, TransportPacket)>,
}

pub struct HostPacketRouter<I> {
    pub sessions: SessionRegistry,
    input_bridge: Option<StylusInputBridge<I>>,
    keyboard_bridge: Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>>,
    touch_bridge: Option<TouchInputBridge<Box<dyn TouchInjector>>>,
    mouse_bridge: Option<MouseInputBridge<Box<dyn MouseInjector>>>,
    permission_policy: PermissionPolicy,
    next_outbound_sequence: u64,
}

#[derive(Debug, Default)]
pub struct NoopPenInjector;

impl PenInjector for NoopPenInjector {
    fn inject_batch(
        &mut self,
        batch: &glyphray_protocol::StylusInputBatch,
        _mapper: &glyphray_core::CoordinateMapper,
        _pressure: &glyphray_core::PressureMapper,
    ) -> Result<InjectionReport, InputError> {
        Ok(InjectionReport {
            injected_samples: batch.samples.len(),
            used_pen_path: false,
        })
    }
}

impl<I> HostPacketRouter<I>
where
    I: PenInjector,
{
    pub fn new(input_bridge: Option<StylusInputBridge<I>>) -> Self {
        Self::new_with_permission_policy(input_bridge, PermissionPolicy::RequireApproval)
    }

    pub fn new_with_permission_policy(
        input_bridge: Option<StylusInputBridge<I>>,
        permission_policy: PermissionPolicy,
    ) -> Self {
        Self::new_with_input_bridges(input_bridge, None, None, None, permission_policy)
    }

    pub fn new_with_input_bridges(
        input_bridge: Option<StylusInputBridge<I>>,
        keyboard_bridge: Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>>,
        touch_bridge: Option<TouchInputBridge<Box<dyn TouchInjector>>>,
        mouse_bridge: Option<MouseInputBridge<Box<dyn MouseInjector>>>,
        permission_policy: PermissionPolicy,
    ) -> Self {
        Self {
            sessions: SessionRegistry::default(),
            input_bridge,
            keyboard_bridge,
            touch_bridge,
            mouse_bridge,
            permission_policy,
            next_outbound_sequence: 1,
        }
    }

    pub fn approve_peer(&mut self, peer: SocketAddr, device_id: impl Into<String>) {
        self.sessions.approve(peer, device_id);
    }

    pub fn approve_peer_with_response(
        &mut self,
        peer: SocketAddr,
        device_id: impl Into<String>,
    ) -> Result<TransportPacket, BackendError> {
        let device_id = device_id.into();
        self.sessions.approve(peer, device_id.clone());
        self.build_pairing_result(true, Some(device_id), None)
    }

    pub fn reject_peer_with_response(
        &mut self,
        peer: SocketAddr,
        reason: impl Into<String>,
    ) -> Result<TransportPacket, BackendError> {
        let reason = reason.into();
        self.sessions.reject(peer);
        self.build_pairing_result(false, None, Some(reason))
    }

    pub fn build_display_info(
        &mut self,
        displays: Vec<DisplayDescriptor>,
    ) -> Result<TransportPacket, BackendError> {
        let response = Message::DisplayInfo(DisplayInfo { displays });
        let payload = encode_frame(self.next_outbound_sequence, &response)
            .map_err(|err| BackendError::Protocol(err.to_string()))?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence += 1;
        Ok(TransportPacket {
            sequence,
            channel: ChannelKind::Control,
            message_kind: MessageKind::DisplayInfo,
            enqueue_timestamp_us: now_us(),
            payload,
        })
    }

    pub fn session_snapshots(&self) -> Vec<SessionSnapshot> {
        self.sessions.snapshots()
    }

    pub fn route_packet(
        &mut self,
        peer: SocketAddr,
        packet: TransportPacket,
    ) -> Result<RouteOutcome, BackendError> {
        let mut outcome = RouteOutcome::default();
        let now = Instant::now();
        if !self.sessions.contains_peer(peer) && !self.sessions.allow_new_pending_peer(peer, now) {
            outcome
                .events
                .push(BackendEvent::PendingRateLimited { peer });
            outcome.events.push(BackendEvent::PacketIgnored {
                peer,
                reason: "pending peer rate limited".to_string(),
            });
            return Ok(outcome);
        }

        let session = self.sessions.ensure_pending(peer);
        session.last_seen = now;
        session.packets_received += 1;
        if session.packets_received == 1 {
            outcome
                .events
                .push(BackendEvent::SessionDiscovered { peer });
        }

        if session.permission == PermissionState::Rejected {
            outcome.events.push(BackendEvent::PacketIgnored {
                peer,
                reason: "peer was rejected".to_string(),
            });
            return Ok(outcome);
        }

        if packet.message_kind == MessageKind::PairingRequest {
            let frame = decode_frame(&packet.payload)
                .map_err(|err| BackendError::Protocol(err.to_string()))?;
            let Message::PairingRequest(request) = frame.message else {
                return Err(BackendError::Protocol(
                    "pairing payload did not contain PairingRequest".to_string(),
                ));
            };
            let public_key_fingerprint = public_key_fingerprint(&request.one_time_public_key);
            session.device_id = Some(request.device_name.clone());
            session.device_public_key_fingerprint = public_key_fingerprint.clone();
            outcome.events.push(BackendEvent::PairingRequested {
                peer,
                device_name: request.device_name,
                public_key_fingerprint,
            });
            if self.permission_policy == PermissionPolicy::DevAutoApprove {
                let device_id = trusted_device_id(peer);
                session.permission = PermissionState::Approved;
                session.device_id = Some(device_id.clone());
                let response = self.build_pairing_result(true, Some(device_id), None)?;
                outcome.outbound.push((peer, response));
                outcome.events.push(BackendEvent::PeerAutoApproved { peer });
                outcome.events.push(BackendEvent::PairingResultQueued {
                    peer,
                    accepted: true,
                });
            }
            return Ok(outcome);
        }

        if session.permission != PermissionState::Approved {
            if self.permission_policy == PermissionPolicy::DevAutoApprove {
                session.permission = PermissionState::Approved;
                session
                    .device_id
                    .get_or_insert_with(|| format!("dev-peer-{peer}"));
                outcome.events.push(BackendEvent::PeerAutoApproved { peer });
            } else {
                outcome
                    .events
                    .push(BackendEvent::PermissionRequired { peer });
                return Ok(outcome);
            }
        }

        if session.permission != PermissionState::Approved {
            outcome
                .events
                .push(BackendEvent::PermissionRequired { peer });
            return Ok(outcome);
        }

        match packet.message_kind {
            MessageKind::StylusInputBatch => {
                let batch = decode_stylus_batch(&packet.payload)?;
                let samples = batch.samples.len();
                if !self.sessions.accept_input_watermark(
                    peer,
                    packet.sequence,
                    batch.monotonic_timestamp_us,
                ) {
                    outcome.events.push(late_packet_event(peer));
                    return Ok(outcome);
                }
                if let Some(bridge) = self.input_bridge.as_mut() {
                    let report = bridge.inject_remote_batch(&batch)?;
                    outcome.events.push(BackendEvent::StylusInjected {
                        peer,
                        samples: report.injected_samples,
                    });
                } else {
                    outcome
                        .events
                        .push(BackendEvent::StylusDecoded { peer, samples });
                }
            }
            MessageKind::LatencyPing => {
                let pong = self.build_latency_pong(&packet.payload)?;
                outcome.outbound.push((peer, pong));
                outcome
                    .events
                    .push(BackendEvent::LatencyPongQueued { peer });
            }
            MessageKind::EncoderConfig => {
                let config = decode_encoder_config(&packet.payload)?;
                let width = config.width;
                let height = config.height;
                let max_fps = config.max_fps;
                let target_bitrate_kbps = config.target_bitrate_kbps;
                self.sessions.ensure_pending(peer).encoder_config = Some(config);
                outcome.events.push(BackendEvent::EncoderConfigUpdated {
                    peer,
                    width,
                    height,
                    max_fps,
                    target_bitrate_kbps,
                });
            }
            MessageKind::KeyboardInput => {
                let keyboard = decode_keyboard_input(&packet.payload)?;
                if !self.sessions.accept_input_watermark(
                    peer,
                    packet.sequence,
                    keyboard.timestamp_us,
                ) {
                    outcome.events.push(late_packet_event(peer));
                    return Ok(outcome);
                }
                outcome.events.push(BackendEvent::KeyboardDecoded {
                    peer,
                    virtual_key: keyboard.virtual_key,
                    pressed: keyboard.pressed,
                });
                if let Some(bridge) = self.keyboard_bridge.as_mut() {
                    bridge.inject_remote_key(&keyboard)?;
                    outcome.events.push(BackendEvent::KeyboardInjected {
                        peer,
                        virtual_key: keyboard.virtual_key,
                        pressed: keyboard.pressed,
                    });
                }
            }
            MessageKind::TouchInputBatch => {
                let batch = decode_touch_input_batch(&packet.payload)?;
                let samples = batch.samples.len();
                if !self.sessions.accept_input_watermark(
                    peer,
                    packet.sequence,
                    batch.monotonic_timestamp_us,
                ) {
                    outcome.events.push(late_packet_event(peer));
                    return Ok(outcome);
                }
                outcome
                    .events
                    .push(BackendEvent::TouchDecoded { peer, samples });
                if let Some(bridge) = self.touch_bridge.as_mut() {
                    bridge.inject_remote_touch_batch(&batch)?;
                    outcome
                        .events
                        .push(BackendEvent::TouchInjected { peer, samples });
                }
            }
            MessageKind::MouseInput => {
                let mouse = decode_mouse_input(&packet.payload)?;
                if !self
                    .sessions
                    .accept_input_watermark(peer, packet.sequence, mouse.timestamp_us)
                {
                    outcome.events.push(late_packet_event(peer));
                    return Ok(outcome);
                }
                outcome.events.push(BackendEvent::MouseDecoded {
                    peer,
                    button_flags: mouse.button_flags,
                });
                if let Some(bridge) = self.mouse_bridge.as_mut() {
                    let report = bridge.inject_remote_mouse(&mouse)?;
                    outcome.events.push(BackendEvent::MouseInjected {
                        peer,
                        injected_events: report.injected_events,
                    });
                }
            }
            MessageKind::GamepadInput => {
                let gamepad = decode_gamepad_input(&packet.payload)?;
                if !self.sessions.accept_input_watermark(
                    peer,
                    packet.sequence,
                    gamepad.timestamp_us,
                ) {
                    outcome.events.push(late_packet_event(peer));
                    return Ok(outcome);
                }
                outcome.events.push(BackendEvent::GamepadDecoded {
                    peer,
                    controller_id: gamepad.controller_id,
                    buttons: gamepad.buttons,
                });
            }
            _ => outcome.events.push(BackendEvent::PacketRouted {
                peer,
                channel: packet.channel,
            }),
        }

        Ok(outcome)
    }

    fn build_latency_pong(&mut self, payload: &[u8]) -> Result<TransportPacket, BackendError> {
        let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
        let Message::LatencyPing(LatencyPing {
            sequence,
            client_send_timestamp_us,
        }) = frame.message
        else {
            return Err(BackendError::Protocol(
                "latency payload did not contain LatencyPing".to_string(),
            ));
        };

        let now = now_us();
        let response = Message::LatencyPong(LatencyPong {
            sequence,
            client_send_timestamp_us,
            host_receive_timestamp_us: now,
            host_send_timestamp_us: now,
        });
        let payload = encode_frame(self.next_outbound_sequence, &response)
            .map_err(|err| BackendError::Protocol(err.to_string()))?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence += 1;
        Ok(TransportPacket {
            sequence,
            channel: ChannelKind::Control,
            message_kind: MessageKind::LatencyPong,
            enqueue_timestamp_us: now,
            payload,
        })
    }

    fn build_pairing_result(
        &mut self,
        accepted: bool,
        trusted_device_id: Option<String>,
        reason: Option<String>,
    ) -> Result<TransportPacket, BackendError> {
        let response = Message::PairingResult(PairingResult {
            accepted,
            trusted_device_id,
            reason,
        });
        let payload = encode_frame(self.next_outbound_sequence, &response)
            .map_err(|err| BackendError::Protocol(err.to_string()))?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence += 1;
        Ok(TransportPacket {
            sequence,
            channel: ChannelKind::Control,
            message_kind: MessageKind::PairingResult,
            enqueue_timestamp_us: now_us(),
            payload,
        })
    }
}

pub struct HostBackendRuntime<I> {
    config: HostConfig,
    advertisement: HostAdvertisement,
    router: HostPacketRouter<I>,
    outbound: OutboundPacketQueues,
    metrics: BackendMetrics,
}

impl<I> HostBackendRuntime<I>
where
    I: PenInjector,
{
    pub fn new(config: HostConfig, input_bridge: Option<StylusInputBridge<I>>) -> Self {
        Self::new_with_permission_policy(config, input_bridge, PermissionPolicy::RequireApproval)
    }

    pub fn new_with_permission_policy(
        config: HostConfig,
        input_bridge: Option<StylusInputBridge<I>>,
        permission_policy: PermissionPolicy,
    ) -> Self {
        Self::new_with_input_bridges(config, input_bridge, None, None, None, permission_policy)
    }

    pub fn new_with_input_bridges(
        config: HostConfig,
        input_bridge: Option<StylusInputBridge<I>>,
        keyboard_bridge: Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>>,
        touch_bridge: Option<TouchInputBridge<Box<dyn TouchInjector>>>,
        mouse_bridge: Option<MouseInputBridge<Box<dyn MouseInjector>>>,
        permission_policy: PermissionPolicy,
    ) -> Self {
        let advertisement = HostAdvertisement {
            host_id: host_id_from_name(&config.host_name),
            host_name: config.host_name.clone(),
            protocol_version: glyphray_protocol::WIRE_VERSION,
            control_port: config.control_port,
            video_port: config.video_port,
            supports_windows_ink: true,
            supports_h264: true,
            pairing_required: config.require_connection_permission,
            load_percent: 0,
        };
        Self {
            config,
            advertisement,
            router: HostPacketRouter::new_with_input_bridges(
                input_bridge,
                keyboard_bridge,
                touch_bridge,
                mouse_bridge,
                permission_policy,
            ),
            outbound: OutboundPacketQueues::default(),
            metrics: BackendMetrics::default(),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn approve_peer(&mut self, peer: SocketAddr, device_id: impl Into<String>) {
        self.router.approve_peer(peer, device_id);
    }

    pub fn approve_peer_and_notify(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let device_id = trusted_device_id(peer);
        self.approve_peer_as_and_notify(server, peer, device_id)
    }

    pub fn approve_peer_as_and_notify(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
        device_id: impl Into<String>,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let device_id = device_id.into();
        let response = self.router.approve_peer_with_response(peer, device_id)?;
        server.send_to(&response, peer)?;
        let displays = current_displays();
        let display_count = displays.len();
        let display_packet = self.router.build_display_info(displays)?;
        server.send_to(&display_packet, peer)?;
        Ok(vec![
            BackendEvent::PeerApproved { peer },
            BackendEvent::PairingResultQueued {
                peer,
                accepted: true,
            },
            BackendEvent::DisplayInfoQueued {
                peer,
                displays: display_count,
            },
        ])
    }

    pub fn reject_peer_and_notify(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
        reason: impl Into<String>,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let response = self.router.reject_peer_with_response(peer, reason)?;
        server.send_to(&response, peer)?;
        Ok(vec![
            BackendEvent::PeerRejected { peer },
            BackendEvent::PairingResultQueued {
                peer,
                accepted: false,
            },
        ])
    }

    pub fn poll_control(
        &mut self,
        server: &mut UdpServer,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let mut events = self.flush_outbound_control(server)?;
        let Some((packet, peer)) = server.poll_recv_from()? else {
            return Ok(events);
        };
        self.metrics.received_packets += 1;

        let mut outcome = self.router.route_packet(peer, packet)?;
        if should_send_display_info(&outcome.events) {
            let displays = current_displays();
            let display_count = displays.len();
            let display_packet = self.router.build_display_info(displays)?;
            outcome.outbound.push((peer, display_packet));
            outcome.events.push(BackendEvent::DisplayInfoQueued {
                peer,
                displays: display_count,
            });
        }
        let outbound_count = outcome.outbound.len();
        if outbound_count > 0 {
            self.enqueue_outbound(outcome.outbound);
            self.metrics.queued_outbound_packets += outbound_count as u64;
            outcome.events.push(BackendEvent::OutboundQueued {
                peer,
                packets: outbound_count,
            });
        }
        self.record_route_events(&outcome.events);
        events.extend(outcome.events);
        events.extend(self.flush_outbound_control(server)?);
        Ok(events)
    }

    pub fn session_count(&self) -> usize {
        self.router.sessions.len()
    }

    pub fn session_snapshots(&self) -> Vec<SessionSnapshot> {
        self.router.session_snapshots()
    }

    pub fn approved_peers(&self) -> Vec<SocketAddr> {
        self.router.sessions.approved_peers()
    }

    pub fn latest_approved_encoder_config(&self) -> Option<EncoderConfig> {
        self.router
            .session_snapshots()
            .into_iter()
            .filter(|session| session.permission == PermissionState::Approved)
            .filter_map(|session| session.encoder_config)
            .last()
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }

    pub fn queue_video_packets_for_approved_peers(
        &mut self,
        packets: Vec<TransportPacket>,
    ) -> Vec<BackendEvent> {
        if packets.is_empty() {
            return Vec::new();
        }

        let peers = self.approved_peers();
        if peers.is_empty() {
            return Vec::new();
        }

        let packet_count = packets.len();
        for peer in &peers {
            for packet in &packets {
                self.outbound.push((*peer, packet.clone()));
            }
        }
        let queued = peers.len() * packet_count;
        self.metrics.queued_outbound_packets += queued as u64;
        self.metrics.queued_video_packets += queued as u64;
        vec![BackendEvent::VideoFrameQueued {
            peers: peers.len(),
            packets: queued,
        }]
    }

    pub fn health_snapshot(&self) -> BackendHealthSnapshot {
        BackendHealthSnapshot {
            sessions_total: self.router.sessions.len(),
            pending_sessions: self.router.sessions.pending_count(),
            outbound: self.outbound.snapshot(),
            metrics: self.metrics,
        }
    }

    pub fn flush_outbound(
        &mut self,
        server: &mut UdpServer,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        self.flush_outbound_control(server)
    }

    fn enqueue_outbound(&mut self, packets: Vec<(SocketAddr, TransportPacket)>) {
        for packet in packets {
            self.outbound.push(packet);
        }
    }

    fn record_route_events(&mut self, events: &[BackendEvent]) {
        for event in events {
            match event {
                BackendEvent::PendingRateLimited { .. } => {
                    self.metrics.pending_rate_limited_packets += 1;
                }
                BackendEvent::PacketIgnored { reason, .. }
                    if reason == LATE_INPUT_PACKET_REASON =>
                {
                    self.metrics.late_input_dropped_packets += 1;
                }
                _ => {}
            }
        }
    }

    fn flush_outbound_control(
        &mut self,
        server: &mut UdpServer,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let mut events = Vec::new();
        for _ in 0..OUTBOUND_FLUSH_BUDGET {
            let Some((peer, packet)) = self.outbound.pop_next() else {
                break;
            };
            if server.try_send_to(&packet, peer)? {
                self.metrics.sent_outbound_packets += 1;
                continue;
            }
            self.outbound.push_front((peer, packet));
            self.metrics.backpressure_events += 1;
            events.push(BackendEvent::OutboundBackpressure {
                peer,
                queued_packets: self.outbound.len(),
            });
            break;
        }
        Ok(events)
    }
}

#[derive(Debug, Default)]
struct OutboundPacketQueues {
    input: VecDeque<(SocketAddr, TransportPacket)>,
    control: VecDeque<(SocketAddr, TransportPacket)>,
    audio: VecDeque<(SocketAddr, TransportPacket)>,
    video: VecDeque<(SocketAddr, TransportPacket)>,
    qos_cursor: usize,
    dropped_packets_total: u64,
    high_watermark: usize,
}

impl OutboundPacketQueues {
    fn push(&mut self, packet: (SocketAddr, TransportPacket)) {
        let dropped = {
            let queue = self.queue_mut(packet.1.channel);
            let dropped = queue.len() == MAX_OUTBOUND_QUEUE_PER_CHANNEL;
            if dropped {
                queue.pop_front();
            }
            queue.push_back(packet);
            dropped
        };
        if dropped {
            self.dropped_packets_total += 1;
        }
        self.update_high_watermark();
    }

    fn push_front(&mut self, packet: (SocketAddr, TransportPacket)) {
        let dropped = {
            let queue = self.queue_mut(packet.1.channel);
            let dropped = queue.len() == MAX_OUTBOUND_QUEUE_PER_CHANNEL;
            if dropped {
                queue.pop_back();
            }
            queue.push_front(packet);
            dropped
        };
        if dropped {
            self.dropped_packets_total += 1;
        }
        self.update_high_watermark();
    }

    fn pop_next(&mut self) -> Option<(SocketAddr, TransportPacket)> {
        for offset in 0..OUTBOUND_QOS_SCHEDULE.len() {
            let index = (self.qos_cursor + offset) % OUTBOUND_QOS_SCHEDULE.len();
            let channel = OUTBOUND_QOS_SCHEDULE[index];
            if let Some(packet) = self.queue_mut(channel).pop_front() {
                self.qos_cursor = (index + 1) % OUTBOUND_QOS_SCHEDULE.len();
                return Some(packet);
            }
        }
        None
    }

    fn len(&self) -> usize {
        self.input.len() + self.control.len() + self.audio.len() + self.video.len()
    }

    fn snapshot(&self) -> OutboundQueueSnapshot {
        OutboundQueueSnapshot {
            input: self.input.len(),
            control: self.control.len(),
            audio: self.audio.len(),
            video: self.video.len(),
            total: self.len(),
            capacity_per_channel: MAX_OUTBOUND_QUEUE_PER_CHANNEL,
            dropped_packets_total: self.dropped_packets_total,
            high_watermark: self.high_watermark,
        }
    }

    fn update_high_watermark(&mut self) {
        self.high_watermark = self.high_watermark.max(self.len());
    }

    fn queue_mut(&mut self, channel: ChannelKind) -> &mut VecDeque<(SocketAddr, TransportPacket)> {
        match channel {
            ChannelKind::Input => &mut self.input,
            ChannelKind::Control => &mut self.control,
            ChannelKind::Audio => &mut self.audio,
            ChannelKind::Video => &mut self.video,
        }
    }
}

fn host_id_from_name(name: &str) -> [u8; 16] {
    let mut id = [0_u8; 16];
    let hash = crc32fast::hash(name.as_bytes());
    id[0..4].copy_from_slice(&hash.to_le_bytes());
    id[4..12].copy_from_slice(&(name.len() as u64).to_le_bytes());
    let checksum = crc32fast::hash(&id[0..12]);
    id[12..16].copy_from_slice(&checksum.to_le_bytes());
    id
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or_default()
}

pub fn trusted_device_id(peer: SocketAddr) -> String {
    format!("trusted-{}", peer).replace([':', '.'], "-")
}

pub fn trusted_device_id_from_public_key_fingerprint(fingerprint: &str) -> String {
    format!("trusted-key-{fingerprint}")
}

fn public_key_fingerprint(public_key: &[u8]) -> Option<String> {
    if public_key.is_empty() {
        return None;
    }
    let digest = Sha256::digest(public_key);
    Some(hex_lower(&digest))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn late_packet_event(peer: SocketAddr) -> BackendEvent {
    BackendEvent::PacketIgnored {
        peer,
        reason: LATE_INPUT_PACKET_REASON.to_string(),
    }
}

fn should_send_display_info(events: &[BackendEvent]) -> bool {
    events.iter().any(|event| {
        matches!(
            event,
            BackendEvent::PairingResultQueued { accepted: true, .. }
        )
    })
}

fn current_displays() -> Vec<DisplayDescriptor> {
    WindowsGraphicsCaptureBackend
        .list_displays()
        .unwrap_or_else(|_| Vec::new())
}

fn decode_encoder_config(payload: &[u8]) -> Result<EncoderConfig, BackendError> {
    let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
    let Message::EncoderConfig(config) = frame.message else {
        return Err(BackendError::Protocol(
            "encoder payload did not contain EncoderConfig".to_string(),
        ));
    };
    Ok(config)
}

fn decode_keyboard_input(payload: &[u8]) -> Result<KeyboardInput, BackendError> {
    let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
    let Message::KeyboardInput(input) = frame.message else {
        return Err(BackendError::Protocol(
            "keyboard payload did not contain KeyboardInput".to_string(),
        ));
    };
    Ok(input)
}

fn decode_touch_input_batch(payload: &[u8]) -> Result<TouchInputBatch, BackendError> {
    let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
    let Message::TouchInputBatch(batch) = frame.message else {
        return Err(BackendError::Protocol(
            "touch payload did not contain TouchInputBatch".to_string(),
        ));
    };
    Ok(batch)
}

fn decode_mouse_input(payload: &[u8]) -> Result<MouseInput, BackendError> {
    let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
    let Message::MouseInput(input) = frame.message else {
        return Err(BackendError::Protocol(
            "mouse payload did not contain MouseInput".to_string(),
        ));
    };
    Ok(input)
}

fn decode_gamepad_input(payload: &[u8]) -> Result<GamepadInput, BackendError> {
    let frame = decode_frame(payload).map_err(|err| BackendError::Protocol(err.to_string()))?;
    let Message::GamepadInput(input) = frame.message else {
        return Err(BackendError::Protocol(
            "gamepad payload did not contain GamepadInput".to_string(),
        ));
    };
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{
        KeyboardInjectionReport, KeyboardInjector, MouseInjectionReport, MouseInjector,
        PenInjector, TouchInjectionReport, TouchInjector,
    };
    use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
    use glyphray_protocol::stylus_wire::encode_stylus_batch;
    use glyphray_protocol::{
        ColorSpace, EncoderConfig, GamepadInput, KeyboardInput, MouseInput, PairingRequest,
        StylusAction, StylusInputBatch, StylusSample, StylusToolType, TouchAction, TouchInputBatch,
        TouchSample, VideoCodec,
    };

    #[derive(Default)]
    struct RecordingInjector;

    impl PenInjector for RecordingInjector {
        fn inject_batch(
            &mut self,
            batch: &StylusInputBatch,
            _mapper: &CoordinateMapper,
            _pressure: &PressureMapper,
        ) -> Result<InjectionReport, InputError> {
            Ok(InjectionReport {
                injected_samples: batch.samples.len(),
                used_pen_path: true,
            })
        }
    }

    #[derive(Default)]
    struct RecordingKeyboardInjector;

    impl KeyboardInjector for RecordingKeyboardInjector {
        fn inject_key(
            &mut self,
            _input: &KeyboardInput,
        ) -> Result<KeyboardInjectionReport, InputError> {
            Ok(KeyboardInjectionReport { injected_events: 1 })
        }
    }

    #[derive(Default)]
    struct RecordingTouchInjector;

    impl TouchInjector for RecordingTouchInjector {
        fn inject_touch_batch(
            &mut self,
            batch: &TouchInputBatch,
            _mapper: &CoordinateMapper,
        ) -> Result<TouchInjectionReport, InputError> {
            Ok(TouchInjectionReport {
                injected_samples: batch.samples.len(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingMouseInjector;

    impl MouseInjector for RecordingMouseInjector {
        fn inject_mouse(
            &mut self,
            _input: &MouseInput,
            _mapper: &CoordinateMapper,
        ) -> Result<MouseInjectionReport, InputError> {
            Ok(MouseInjectionReport { injected_events: 1 })
        }
    }

    #[test]
    fn unapproved_peer_requires_permission() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50000".parse().expect("peer");
        let outcome = router
            .route_packet(peer, input_packet(vec![1, 2, 3]))
            .expect("route");
        assert!(outcome
            .events
            .contains(&BackendEvent::PermissionRequired { peer }));
    }

    #[test]
    fn pending_sessions_are_capped_by_evicting_oldest_pending_peer() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let first_peer: SocketAddr = "10.0.0.1:51000".parse().expect("peer");

        for host in 1..52 {
            let peer: SocketAddr = format!("10.0.0.{host}:51000").parse().expect("peer");
            router
                .route_packet(peer, input_packet(vec![1, 2, 3]))
                .expect("route");
        }

        assert_eq!(router.sessions.pending_count(), MAX_PENDING_SESSIONS);
        assert!(!router
            .session_snapshots()
            .iter()
            .any(|session| session.peer == first_peer));
    }

    #[test]
    fn pending_attempts_are_rate_limited_per_ip() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);

        for port in 52000..52012 {
            let peer: SocketAddr = format!("127.0.0.1:{port}").parse().expect("peer");
            let outcome = router
                .route_packet(peer, input_packet(vec![1, 2, 3]))
                .expect("route");
            assert!(outcome
                .events
                .contains(&BackendEvent::PermissionRequired { peer }));
        }

        let limited_peer: SocketAddr = "127.0.0.1:52012".parse().expect("peer");
        let outcome = router
            .route_packet(limited_peer, input_packet(vec![1, 2, 3]))
            .expect("route");

        assert!(outcome
            .events
            .contains(&BackendEvent::PendingRateLimited { peer: limited_peer }));
        assert_eq!(router.sessions.pending_count(), MAX_PENDING_ATTEMPTS_PER_IP);
    }

    #[test]
    fn approved_stylus_packet_reaches_injector() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        );
        let bridge = StylusInputBridge::new(RecordingInjector, mapper, PressureMapper::default());
        let mut router = HostPacketRouter::new(Some(bridge));
        let peer: SocketAddr = "127.0.0.1:50001".parse().expect("peer");
        router.approve_peer(peer, "tablet");
        let payload = encode_stylus_batch(&sample_batch()).expect("encode");
        let outcome = router
            .route_packet(peer, input_packet(payload))
            .expect("route");
        assert!(outcome
            .events
            .contains(&BackendEvent::StylusInjected { peer, samples: 1 }));
    }

    #[test]
    fn late_stylus_packet_is_dropped_before_injection() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50011".parse().expect("peer");
        router.approve_peer(peer, "tablet");

        let first = router
            .route_packet(peer, input_packet_with_sequence(2, sample_batch_at(20)))
            .expect("route first");
        assert!(first
            .events
            .contains(&BackendEvent::StylusDecoded { peer, samples: 1 }));

        let late = router
            .route_packet(peer, input_packet_with_sequence(1, sample_batch_at(10)))
            .expect("route late");
        assert!(late.events.contains(&BackendEvent::PacketIgnored {
            peer,
            reason: "late input packet".to_string(),
        }));
    }

    #[test]
    fn pending_peer_can_request_pairing() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50002".parse().expect("peer");
        let message = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: vec![1, 2, 3],
            one_time_public_key: vec![4, 5, 6],
        });
        let packet = TransportPacket {
            sequence: 1,
            channel: ChannelKind::Control,
            message_kind: MessageKind::PairingRequest,
            enqueue_timestamp_us: 0,
            payload: encode_frame(1, &message).expect("encode"),
        };

        let outcome = router.route_packet(peer, packet).expect("route");
        let expected_fingerprint = public_key_fingerprint(&[4, 5, 6]).expect("fingerprint");
        assert!(outcome.events.contains(&BackendEvent::PairingRequested {
            peer,
            device_name: "Galaxy Tab".to_string(),
            public_key_fingerprint: Some(expected_fingerprint),
        }));
    }

    #[test]
    fn dev_auto_approve_allows_input_without_manual_permission() {
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_permission_policy(
            None,
            PermissionPolicy::DevAutoApprove,
        );
        let peer: SocketAddr = "127.0.0.1:50003".parse().expect("peer");
        let payload = encode_stylus_batch(&sample_batch()).expect("encode");
        let outcome = router
            .route_packet(peer, input_packet(payload))
            .expect("route");
        assert!(outcome
            .events
            .contains(&BackendEvent::PeerAutoApproved { peer }));
        assert!(outcome
            .events
            .contains(&BackendEvent::StylusDecoded { peer, samples: 1 }));
    }

    #[test]
    fn dev_auto_approve_pairing_queues_pairing_result() {
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_permission_policy(
            None,
            PermissionPolicy::DevAutoApprove,
        );
        let peer: SocketAddr = "127.0.0.1:50004".parse().expect("peer");
        let message = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: vec![],
            one_time_public_key: vec![],
        });
        let packet = TransportPacket {
            sequence: 1,
            channel: ChannelKind::Control,
            message_kind: MessageKind::PairingRequest,
            enqueue_timestamp_us: 0,
            payload: encode_frame(1, &message).expect("encode"),
        };

        let outcome = router.route_packet(peer, packet).expect("route");

        assert!(outcome
            .events
            .contains(&BackendEvent::PeerAutoApproved { peer }));
        assert!(outcome.events.contains(&BackendEvent::PairingResultQueued {
            peer,
            accepted: true,
        }));
        assert_eq!(outcome.outbound.len(), 1);
        assert_eq!(
            outcome.outbound[0].1.message_kind,
            MessageKind::PairingResult
        );
    }

    #[test]
    fn display_info_packet_contains_monitor_descriptors() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let packet = router
            .build_display_info(vec![DisplayDescriptor {
                id: 1,
                name: "Primary".to_string(),
                origin_x: 0,
                origin_y: 0,
                width_px: 1920,
                height_px: 1080,
                scale_factor: 1.0,
                rotation_degrees: 0,
                refresh_hz: 60.0,
                primary: true,
            }])
            .expect("display info");

        assert_eq!(packet.message_kind, MessageKind::DisplayInfo);
        let frame = decode_frame(&packet.payload).expect("decode");
        let Message::DisplayInfo(info) = frame.message else {
            panic!("expected display info");
        };
        assert_eq!(info.displays.len(), 1);
        assert_eq!(info.displays[0].name, "Primary");
        assert!(info.displays[0].primary);
    }

    #[test]
    fn approved_peer_can_update_encoder_config() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50005".parse().expect("peer");
        router.approve_peer(peer, "tablet");
        let message = Message::EncoderConfig(EncoderConfig {
            display_id: 0,
            codec: VideoCodec::H264,
            color_space: ColorSpace::Rec709,
            width: 2560,
            height: 1440,
            max_fps: 120,
            target_bitrate_kbps: 35_000,
            keyframe_interval_ms: 1_000,
            low_latency: true,
        });
        let outcome = router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 1,
                    channel: ChannelKind::Control,
                    message_kind: MessageKind::EncoderConfig,
                    enqueue_timestamp_us: 0,
                    payload: encode_frame(1, &message).expect("encode"),
                },
            )
            .expect("route");

        assert!(outcome
            .events
            .contains(&BackendEvent::EncoderConfigUpdated {
                peer,
                width: 2560,
                height: 1440,
                max_fps: 120,
                target_bitrate_kbps: 35_000,
            }));
        assert_eq!(
            router.session_snapshots()[0]
                .encoder_config
                .as_ref()
                .expect("encoder config")
                .color_space,
            ColorSpace::Rec709
        );
    }

    #[test]
    fn approved_peer_keyboard_packet_is_decoded() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50006".parse().expect("peer");
        router.approve_peer(peer, "keyboard");
        let message = Message::KeyboardInput(KeyboardInput {
            sequence: 1,
            timestamp_us: 22,
            scan_code: 0,
            virtual_key: 0x5B,
            pressed: true,
            modifiers: 0,
        });
        let outcome = router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 1,
                    channel: ChannelKind::Input,
                    message_kind: MessageKind::KeyboardInput,
                    enqueue_timestamp_us: 0,
                    payload: encode_frame(1, &message).expect("encode"),
                },
            )
            .expect("route");

        assert!(outcome.events.contains(&BackendEvent::KeyboardDecoded {
            peer,
            virtual_key: 0x5B,
            pressed: true,
        }));
    }

    #[test]
    fn approved_peer_keyboard_packet_can_be_injected() {
        let keyboard_bridge = KeyboardInputBridge::new(
            Box::new(RecordingKeyboardInjector) as Box<dyn KeyboardInjector>
        );
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_input_bridges(
            None,
            Some(keyboard_bridge),
            None,
            None,
            PermissionPolicy::RequireApproval,
        );
        let peer: SocketAddr = "127.0.0.1:50007".parse().expect("peer");
        router.approve_peer(peer, "keyboard");
        let message = Message::KeyboardInput(KeyboardInput {
            sequence: 1,
            timestamp_us: 22,
            scan_code: 0x37,
            virtual_key: 0x2C,
            pressed: true,
            modifiers: 0,
        });
        let outcome = router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 1,
                    channel: ChannelKind::Input,
                    message_kind: MessageKind::KeyboardInput,
                    enqueue_timestamp_us: 0,
                    payload: encode_frame(1, &message).expect("encode"),
                },
            )
            .expect("route");

        assert!(outcome.events.contains(&BackendEvent::KeyboardInjected {
            peer,
            virtual_key: 0x2C,
            pressed: true,
        }));
    }

    #[test]
    fn approved_peer_touch_packet_can_be_injected() {
        let mapper = test_mapper();
        let touch_bridge = TouchInputBridge::new(
            Box::new(RecordingTouchInjector) as Box<dyn TouchInjector>,
            mapper,
        );
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_input_bridges(
            None,
            None,
            Some(touch_bridge),
            None,
            PermissionPolicy::RequireApproval,
        );
        let peer: SocketAddr = "127.0.0.1:50008".parse().expect("peer");
        router.approve_peer(peer, "touch");
        let message = Message::TouchInputBatch(sample_touch_batch());

        let outcome = router
            .route_packet(
                peer,
                framed_input_packet(MessageKind::TouchInputBatch, message),
            )
            .expect("route");

        assert!(outcome
            .events
            .contains(&BackendEvent::TouchInjected { peer, samples: 1 }));
    }

    #[test]
    fn approved_peer_mouse_packet_can_be_injected() {
        let mouse_bridge = MouseInputBridge::new(
            Box::new(RecordingMouseInjector) as Box<dyn MouseInjector>,
            test_mapper(),
        );
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_input_bridges(
            None,
            None,
            None,
            Some(mouse_bridge),
            PermissionPolicy::RequireApproval,
        );
        let peer: SocketAddr = "127.0.0.1:50009".parse().expect("peer");
        router.approve_peer(peer, "mouse");
        let message = Message::MouseInput(MouseInput {
            sequence: 1,
            timestamp_us: 22,
            display_id: 0,
            x: 44.0,
            y: 55.0,
            wheel_delta_x: 0.0,
            wheel_delta_y: 1.0,
            button_flags: 1,
        });

        let outcome = router
            .route_packet(peer, framed_input_packet(MessageKind::MouseInput, message))
            .expect("route");

        assert!(outcome.events.contains(&BackendEvent::MouseInjected {
            peer,
            injected_events: 1,
        }));
    }

    #[test]
    fn approved_peer_gamepad_packet_is_decoded() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50010".parse().expect("peer");
        router.approve_peer(peer, "gamepad");
        let message = Message::GamepadInput(GamepadInput {
            sequence: 1,
            timestamp_us: 22,
            controller_id: 7,
            connected: true,
            buttons: 0b11,
            left_trigger: 0.0,
            right_trigger: 1.0,
            left_stick_x: 0.2,
            left_stick_y: -0.3,
            right_stick_x: 0.4,
            right_stick_y: -0.5,
        });

        let outcome = router
            .route_packet(
                peer,
                framed_input_packet(MessageKind::GamepadInput, message),
            )
            .expect("route");

        assert!(outcome.events.contains(&BackendEvent::GamepadDecoded {
            peer,
            controller_id: 7,
            buttons: 0b11,
        }));
    }

    #[test]
    fn host_id_uses_crc_based_hashing() {
        let first = host_id_from_name("GlyphRay Host A");
        let second = host_id_from_name("GlyphRay Host B");

        assert_ne!(first, [0_u8; 16]);
        assert_ne!(first, second);
        assert_eq!(first, host_id_from_name("GlyphRay Host A"));
    }

    #[test]
    fn outbound_qos_prefers_control_over_video_backlog() {
        let peer: SocketAddr = "127.0.0.1:53000".parse().expect("peer");
        let mut queue = OutboundPacketQueues::default();

        for sequence in 1..5 {
            queue.push((peer, packet_with_channel(sequence, ChannelKind::Video)));
        }
        queue.push((peer, packet_with_channel(99, ChannelKind::Control)));

        let (_, first) = queue.pop_next().expect("first packet");
        assert_eq!(first.channel, ChannelKind::Control);
        assert_eq!(first.sequence, 99);
    }

    #[test]
    fn outbound_queue_snapshot_tracks_lengths_and_drops() {
        let peer: SocketAddr = "127.0.0.1:53001".parse().expect("peer");
        let mut queue = OutboundPacketQueues::default();

        for sequence in 0..(MAX_OUTBOUND_QUEUE_PER_CHANNEL as u64 + 2) {
            queue.push((peer, packet_with_channel(sequence, ChannelKind::Video)));
        }

        let snapshot = queue.snapshot();
        assert_eq!(snapshot.video, MAX_OUTBOUND_QUEUE_PER_CHANNEL);
        assert_eq!(snapshot.total, MAX_OUTBOUND_QUEUE_PER_CHANNEL);
        assert_eq!(
            snapshot.capacity_per_channel,
            MAX_OUTBOUND_QUEUE_PER_CHANNEL
        );
        assert_eq!(snapshot.dropped_packets_total, 2);
        assert_eq!(snapshot.high_watermark, MAX_OUTBOUND_QUEUE_PER_CHANNEL);
    }

    #[test]
    fn runtime_health_snapshot_reports_pending_sessions_and_queue_state() {
        let mut runtime = HostBackendRuntime::<RecordingInjector>::new(HostConfig::default(), None);
        let peer: SocketAddr = "127.0.0.1:53002".parse().expect("peer");

        runtime
            .router
            .route_packet(peer, input_packet(vec![1, 2, 3]))
            .expect("route");
        runtime
            .outbound
            .push((peer, packet_with_channel(1, ChannelKind::Control)));

        let snapshot = runtime.health_snapshot();
        assert_eq!(snapshot.sessions_total, 1);
        assert_eq!(snapshot.pending_sessions, 1);
        assert_eq!(snapshot.outbound.control, 1);
        assert_eq!(snapshot.outbound.total, 1);
    }

    #[test]
    fn runtime_queues_video_packets_for_approved_peers_only() {
        let mut runtime = HostBackendRuntime::<RecordingInjector>::new(HostConfig::default(), None);
        let approved: SocketAddr = "127.0.0.1:53004".parse().expect("peer");
        let pending: SocketAddr = "127.0.0.1:53005".parse().expect("peer");
        runtime.approve_peer(approved, "tablet");
        runtime
            .router
            .route_packet(pending, input_packet(vec![1, 2, 3]))
            .expect("route");

        let events = runtime.queue_video_packets_for_approved_peers(vec![
            packet_with_channel(10, ChannelKind::Video),
            packet_with_channel(11, ChannelKind::Video),
        ]);
        let snapshot = runtime.health_snapshot();

        assert_eq!(
            events,
            vec![BackendEvent::VideoFrameQueued {
                peers: 1,
                packets: 2,
            }]
        );
        assert_eq!(snapshot.outbound.video, 2);
        assert_eq!(snapshot.metrics.queued_video_packets, 2);
    }

    #[test]
    fn runtime_metrics_count_hardening_events() {
        let mut runtime = HostBackendRuntime::<RecordingInjector>::new(HostConfig::default(), None);
        let peer: SocketAddr = "127.0.0.1:53003".parse().expect("peer");

        runtime.record_route_events(&[
            BackendEvent::PendingRateLimited { peer },
            BackendEvent::PacketIgnored {
                peer,
                reason: LATE_INPUT_PACKET_REASON.to_string(),
            },
        ]);

        let snapshot = runtime.health_snapshot();
        assert_eq!(snapshot.metrics.pending_rate_limited_packets, 1);
        assert_eq!(snapshot.metrics.late_input_dropped_packets, 1);
    }

    fn input_packet(payload: Vec<u8>) -> TransportPacket {
        TransportPacket {
            sequence: 1,
            channel: ChannelKind::Input,
            message_kind: MessageKind::StylusInputBatch,
            enqueue_timestamp_us: 0,
            payload,
        }
    }

    fn packet_with_channel(sequence: u64, channel: ChannelKind) -> TransportPacket {
        TransportPacket {
            sequence,
            channel,
            message_kind: MessageKind::LatencyPing,
            enqueue_timestamp_us: 0,
            payload: vec![sequence as u8],
        }
    }

    fn input_packet_with_sequence(sequence: u64, batch: StylusInputBatch) -> TransportPacket {
        TransportPacket {
            sequence,
            channel: ChannelKind::Input,
            message_kind: MessageKind::StylusInputBatch,
            enqueue_timestamp_us: 0,
            payload: encode_stylus_batch(&batch).expect("encode"),
        }
    }

    fn framed_input_packet(message_kind: MessageKind, message: Message) -> TransportPacket {
        TransportPacket {
            sequence: 1,
            channel: ChannelKind::Input,
            message_kind,
            enqueue_timestamp_us: 0,
            payload: encode_frame(1, &message).expect("encode"),
        }
    }

    fn test_mapper() -> CoordinateMapper {
        CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        )
    }

    fn sample_touch_batch() -> TouchInputBatch {
        TouchInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us: 1,
            display_id: 0,
            samples: vec![TouchSample {
                sequence: 1,
                timestamp_us: 1,
                pointer_id: 1,
                action: TouchAction::Down,
                x: 10.0,
                y: 20.0,
                pressure: 0.5,
                major: 8.0,
                minor: 8.0,
                orientation_degrees: 0.0,
                flags: 0,
            }],
        }
    }

    fn sample_batch() -> StylusInputBatch {
        sample_batch_at(1)
    }

    fn sample_batch_at(monotonic_timestamp_us: u64) -> StylusInputBatch {
        StylusInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us,
            samples: vec![StylusSample {
                sequence: 1,
                timestamp_us: monotonic_timestamp_us,
                display_id: 0,
                pointer_id: 1,
                tool_type: StylusToolType::Stylus,
                action: StylusAction::Move,
                x: 50.0,
                y: 50.0,
                pressure: 0.5,
                tilt_x_degrees: 0.0,
                tilt_y_degrees: 0.0,
                orientation_degrees: 0.0,
                button_flags: 0,
                hover: false,
                eraser: false,
                predicted: false,
            }],
        }
    }
}
