use crate::config::HostConfig;
use crate::input::{InjectionReport, InputError, PenInjector, StylusInputBridge};
use glyphray_protocol::stylus_wire::{decode_stylus_batch, StylusWireError};
use glyphray_protocol::{
    decode_frame, encode_frame, LatencyPing, LatencyPong, Message, MessageKind, PairingResult,
};
use glyphray_transport::discovery::HostAdvertisement;
use glyphray_transport::udp::UdpServer;
use glyphray_transport::{ChannelKind, TransportError, TransportPacket};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    pub permission: PermissionState,
    pub packets_received: u64,
    pub last_seen: Instant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub peer: SocketAddr,
    pub device_id: Option<String>,
    pub permission: PermissionState,
    pub packets_received: u64,
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<SocketAddr, ClientSession>,
}

impl SessionRegistry {
    pub fn ensure_pending(&mut self, peer: SocketAddr) -> &mut ClientSession {
        self.sessions.entry(peer).or_insert_with(|| ClientSession {
            peer,
            device_id: None,
            permission: PermissionState::Pending,
            packets_received: 0,
            last_seen: Instant::now(),
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

    pub fn snapshots(&self) -> Vec<SessionSnapshot> {
        let mut snapshots = self
            .sessions
            .values()
            .map(|session| SessionSnapshot {
                peer: session.peer,
                device_id: session.device_id.clone(),
                permission: session.permission,
                packets_received: session.packets_received,
            })
            .collect::<Vec<_>>();
        snapshots.sort_by_key(|session| session.peer);
        snapshots
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
    },
    PairingResultQueued {
        peer: SocketAddr,
        accepted: bool,
    },
    PermissionRequired {
        peer: SocketAddr,
    },
    PacketIgnored {
        peer: SocketAddr,
        reason: String,
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
        Self {
            sessions: SessionRegistry::default(),
            input_bridge,
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

    pub fn session_snapshots(&self) -> Vec<SessionSnapshot> {
        self.sessions.snapshots()
    }

    pub fn route_packet(
        &mut self,
        peer: SocketAddr,
        packet: TransportPacket,
    ) -> Result<RouteOutcome, BackendError> {
        let mut outcome = RouteOutcome::default();
        let session = self.sessions.ensure_pending(peer);
        session.last_seen = Instant::now();
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
            session.device_id = Some(request.device_name.clone());
            outcome.events.push(BackendEvent::PairingRequested {
                peer,
                device_name: request.device_name,
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
            router: HostPacketRouter::new_with_permission_policy(input_bridge, permission_policy),
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
        let response = self.router.approve_peer_with_response(peer, device_id)?;
        server.send_to(&response, peer)?;
        Ok(vec![
            BackendEvent::PeerApproved { peer },
            BackendEvent::PairingResultQueued {
                peer,
                accepted: true,
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
        let Some((packet, peer)) = server.poll_recv_from()? else {
            return Ok(Vec::new());
        };

        let outcome = self.router.route_packet(peer, packet)?;
        for (peer, packet) in outcome.outbound {
            server.send_to(&packet, peer)?;
        }
        Ok(outcome.events)
    }

    pub fn session_count(&self) -> usize {
        self.router.sessions.len()
    }

    pub fn session_snapshots(&self) -> Vec<SessionSnapshot> {
        self.router.session_snapshots()
    }

    pub fn config(&self) -> &HostConfig {
        &self.config
    }
}

fn host_id_from_name(name: &str) -> [u8; 16] {
    let mut id = [0_u8; 16];
    for (index, byte) in name.as_bytes().iter().enumerate() {
        id[index % 16] = id[index % 16].wrapping_mul(31).wrapping_add(*byte);
    }
    id
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or_default()
}

fn trusted_device_id(peer: SocketAddr) -> String {
    format!("trusted-{}", peer).replace([':', '.'], "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::PenInjector;
    use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
    use glyphray_protocol::stylus_wire::encode_stylus_batch;
    use glyphray_protocol::{
        PairingRequest, StylusAction, StylusInputBatch, StylusSample, StylusToolType,
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
        assert!(outcome.events.contains(&BackendEvent::PairingRequested {
            peer,
            device_name: "Galaxy Tab".to_string()
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

    fn input_packet(payload: Vec<u8>) -> TransportPacket {
        TransportPacket {
            sequence: 1,
            channel: ChannelKind::Input,
            message_kind: MessageKind::StylusInputBatch,
            enqueue_timestamp_us: 0,
            payload,
        }
    }

    fn sample_batch() -> StylusInputBatch {
        StylusInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us: 1,
            samples: vec![StylusSample {
                sequence: 1,
                timestamp_us: 1,
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
