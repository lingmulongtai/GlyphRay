use crate::capture::{ScreenCapture, WindowsGraphicsCaptureBackend};
use crate::config::HostConfig;
use crate::input::{
    GamepadInjector, GamepadInputBridge, InjectionReport, InputError, KeyboardInjector,
    KeyboardInputBridge, MouseInjector, MouseInputBridge, PenInjector, StylusInputBridge,
    TouchInjector, TouchInputBridge,
};
use crate::secrets::HostIdentity;
use crate::settings::TrustedDevicePermissions;
use glyphray_protocol::session_wire::{
    client_signing_payload, decode_client_key_confirm, encode_server_key_exchange,
    server_signing_payload, session_transcript_hash, ClientKeyConfirm, ServerKeyExchange,
    SessionWireError,
};
use glyphray_protocol::stylus_wire::{decode_stylus_batch, StylusWireError};
use glyphray_protocol::{
    decode_frame, encode_frame, trusted_auth_challenge_payload, AuthChallenge, AuthResponse,
    DisplayDescriptor, DisplayInfo, EncoderConfig, GamepadInput, KeyboardInput, LatencyPing,
    LatencyPong, Message, MessageKind, MouseInput, PairingChallenge, PairingResult,
    TouchInputBatch,
};
use glyphray_security::{verify_pairing_code_proof, PairingCode, SecretBytes, SessionCipherPair};
use glyphray_transport::discovery::HostAdvertisement;
use glyphray_transport::secure::SecureDatagramCodec;
use glyphray_transport::udp::{decode_packet, encode_packet, ReceivedDatagram, UdpServer};
use glyphray_transport::{ChannelKind, TransportError, TransportPacket};
use p256::ecdh::EphemeralSecret;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::{DecodePublicKey, EncodePublicKey};
use p256::PublicKey;
use rand::rngs::OsRng;
use rand::RngCore;
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
const TRUSTED_AUTH_CHALLENGE_TTL_MS: u64 = 30_000;
const SESSION_KEY_EXCHANGE_TTL_MS: u64 = 30_000;
const PAIRING_CODE_TTL_MS: u64 = 5 * 60_000;
const PAIRING_CHALLENGE_TTL_MS: u64 = 2 * 60_000;
const MAX_PAIRING_CODE_ATTEMPTS: u8 = 5;
const PAIRING_ATTEMPT_WINDOW_MS: u64 = 2 * 60_000;

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    StylusWire(#[from] StylusWireError),
    #[error(transparent)]
    SessionWire(#[from] SessionWireError),
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

pub struct ClientSession {
    pub peer: SocketAddr,
    pub device_id: Option<String>,
    pub device_public_key_der: Option<Vec<u8>>,
    pub device_public_key_fingerprint: Option<String>,
    pub pending_auth_challenge: Option<PendingAuthChallenge>,
    pending_pairing_challenge: Option<PendingPairingChallenge>,
    pairing_code_attempts: u8,
    pairing_attempt_window_started_unix_ms: u64,
    pairing_code_verified: bool,
    pending_key_exchange: Option<PendingKeyExchange>,
    secure_session: Option<ActiveSecureSession>,
    pub permission: PermissionState,
    pub input_permissions: TrustedDevicePermissions,
    pub packets_received: u64,
    pub last_seen: Instant,
    pub encoder_config: Option<EncoderConfig>,
    pub last_input_sequence: Option<u64>,
    pub last_input_timestamp_us: Option<u64>,
}

struct PendingKeyExchange {
    exchange: ServerKeyExchange,
    ephemeral_secret: EphemeralSecret,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPairingChallenge {
    salt: [u8; 32],
    expires_at_unix_ms: u64,
}

struct ActiveSecureSession {
    session_id: [u8; 16],
    ciphers: SessionCipherPair,
    codec: SecureDatagramCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAuthChallenge {
    pub challenge_id: u64,
    pub nonce: [u8; 32],
    pub issued_at_unix_ms: u64,
    pub expected_device_id: String,
    pub public_key_der: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSnapshot {
    pub peer: SocketAddr,
    pub device_id: Option<String>,
    pub device_public_key_der: Option<Vec<u8>>,
    pub device_public_key_fingerprint: Option<String>,
    pub has_pending_auth_challenge: bool,
    pub pairing_code_verified: bool,
    pub has_pending_key_exchange: bool,
    pub secure: bool,
    pub permission: PermissionState,
    pub input_permissions: TrustedDevicePermissions,
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
    pub queued_audio_packets: u64,
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

#[derive(Default)]
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
            device_public_key_der: None,
            device_public_key_fingerprint: None,
            pending_auth_challenge: None,
            pending_pairing_challenge: None,
            pairing_code_attempts: 0,
            pairing_attempt_window_started_unix_ms: now_ms(),
            pairing_code_verified: false,
            pending_key_exchange: None,
            secure_session: None,
            permission: PermissionState::Pending,
            input_permissions: TrustedDevicePermissions::default(),
            packets_received: 0,
            last_seen: Instant::now(),
            encoder_config: None,
            last_input_sequence: None,
            last_input_timestamp_us: None,
        })
    }

    pub fn approve(&mut self, peer: SocketAddr, device_id: impl Into<String>) {
        self.approve_with_permissions(peer, device_id, TrustedDevicePermissions::default());
    }

    pub fn approve_with_permissions(
        &mut self,
        peer: SocketAddr,
        device_id: impl Into<String>,
        permissions: TrustedDevicePermissions,
    ) {
        let session = self.ensure_pending(peer);
        session.permission = PermissionState::Approved;
        session.device_id = Some(device_id.into());
        session.input_permissions = permissions;
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

    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
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
                device_public_key_der: session.device_public_key_der.clone(),
                device_public_key_fingerprint: session.device_public_key_fingerprint.clone(),
                has_pending_auth_challenge: session.pending_auth_challenge.is_some(),
                pairing_code_verified: session.pairing_code_verified,
                has_pending_key_exchange: session.pending_key_exchange.is_some(),
                secure: session.secure_session.is_some(),
                permission: session.permission,
                input_permissions: session.input_permissions.clone(),
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

    pub fn secure_peers(&self) -> Vec<SocketAddr> {
        let mut peers = self
            .sessions
            .values()
            .filter(|session| {
                session.permission == PermissionState::Approved && session.secure_session.is_some()
            })
            .map(|session| session.peer)
            .collect::<Vec<_>>();
        peers.sort();
        peers
    }

    pub fn requires_secure_transport(&self, peer: SocketAddr) -> bool {
        self.sessions.get(&peer).is_some_and(|session| {
            session.permission == PermissionState::Approved
                && session.pending_key_exchange.is_some()
                && session.secure_session.is_none()
        })
    }

    pub fn allows_input(&self, peer: SocketAddr, message_kind: MessageKind) -> bool {
        let Some(session) = self.sessions.get(&peer) else {
            return false;
        };
        match message_kind {
            MessageKind::StylusInputBatch => session.input_permissions.allow_pen,
            MessageKind::TouchInputBatch => session.input_permissions.allow_touch,
            MessageKind::KeyboardInput => session.input_permissions.allow_keyboard,
            MessageKind::MouseInput => session.input_permissions.allow_mouse,
            MessageKind::GamepadInput => session.input_permissions.allow_gamepad,
            _ => true,
        }
    }

    pub fn update_permissions_for_device(
        &mut self,
        device_id: &str,
        permissions: TrustedDevicePermissions,
    ) -> usize {
        let mut updated = 0;
        for session in self.sessions.values_mut() {
            if session.device_id.as_deref() == Some(device_id) {
                session.input_permissions = permissions.clone();
                updated += 1;
            }
        }
        updated
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
    SessionKeyExchangeQueued {
        peer: SocketAddr,
        session_id: [u8; 16],
    },
    SessionSecured {
        peer: SocketAddr,
        session_id: [u8; 16],
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
        code_verified: bool,
    },
    PairingCodeChallengeQueued {
        peer: SocketAddr,
        display_code: String,
        expires_at_unix_ms: u64,
    },
    PairingCodeRejected {
        peer: SocketAddr,
        attempts_remaining: u8,
        reason: String,
    },
    AuthChallengeQueued {
        peer: SocketAddr,
        challenge_id: u64,
    },
    TrustedDeviceAuthenticated {
        peer: SocketAddr,
        trusted_device_id: String,
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
    GamepadInjected {
        peer: SocketAddr,
        controller_id: u32,
        connected: bool,
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
    AudioFrameQueued {
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
    gamepad_bridge: Option<GamepadInputBridge<Box<dyn GamepadInjector>>>,
    permission_policy: PermissionPolicy,
    next_outbound_sequence: u64,
    host_identity: HostIdentity,
    pairing_code: PairingCode,
    pairing_code_expires_at_unix_ms: u64,
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
        Self::new_with_input_bridges(input_bridge, None, None, None, None, permission_policy)
    }

    pub fn new_with_input_bridges(
        input_bridge: Option<StylusInputBridge<I>>,
        keyboard_bridge: Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>>,
        touch_bridge: Option<TouchInputBridge<Box<dyn TouchInjector>>>,
        mouse_bridge: Option<MouseInputBridge<Box<dyn MouseInjector>>>,
        gamepad_bridge: Option<GamepadInputBridge<Box<dyn GamepadInjector>>>,
        permission_policy: PermissionPolicy,
    ) -> Self {
        Self {
            sessions: SessionRegistry::default(),
            input_bridge,
            keyboard_bridge,
            touch_bridge,
            mouse_bridge,
            gamepad_bridge,
            permission_policy,
            next_outbound_sequence: 1,
            host_identity: HostIdentity::generate(),
            pairing_code: PairingCode::generate(),
            pairing_code_expires_at_unix_ms: now_ms().saturating_add(PAIRING_CODE_TTL_MS),
        }
    }

    pub fn set_host_identity(&mut self, identity: HostIdentity) {
        self.host_identity = identity;
    }

    pub fn is_secure(&self, peer: SocketAddr) -> bool {
        self.sessions
            .sessions
            .get(&peer)
            .is_some_and(|session| session.secure_session.is_some())
    }

    fn open_secure_packet(
        &mut self,
        peer: SocketAddr,
        sealed: &glyphray_security::SealedPacket,
    ) -> Result<TransportPacket, BackendError> {
        let secure = self
            .sessions
            .sessions
            .get_mut(&peer)
            .and_then(|session| session.secure_session.as_mut())
            .ok_or_else(|| {
                BackendError::Protocol("secure session is not established".to_string())
            })?;
        let plaintext = secure.codec.open(&secure.ciphers.inbound, sealed)?;
        Ok(decode_packet(&plaintext)?)
    }

    fn seal_secure_packet(
        &mut self,
        peer: SocketAddr,
        packet: &TransportPacket,
    ) -> Result<glyphray_security::SealedPacket, BackendError> {
        let secure = self
            .sessions
            .sessions
            .get_mut(&peer)
            .and_then(|session| session.secure_session.as_mut())
            .ok_or_else(|| {
                BackendError::Protocol("secure session is not established".to_string())
            })?;
        debug_assert_ne!(secure.session_id, [0_u8; 16]);
        let plaintext = encode_packet(packet)?;
        Ok(secure.codec.seal(&secure.ciphers.outbound, &plaintext)?)
    }

    pub fn approve_peer(&mut self, peer: SocketAddr, device_id: impl Into<String>) {
        self.sessions.approve(peer, device_id);
    }

    pub fn approve_peer_with_permissions(
        &mut self,
        peer: SocketAddr,
        device_id: impl Into<String>,
        permissions: TrustedDevicePermissions,
    ) {
        self.sessions
            .approve_with_permissions(peer, device_id, permissions);
    }

    pub fn approve_peer_with_response(
        &mut self,
        peer: SocketAddr,
        device_id: impl Into<String>,
    ) -> Result<Vec<TransportPacket>, BackendError> {
        self.approve_peer_with_response_and_permissions(
            peer,
            device_id,
            TrustedDevicePermissions::default(),
        )
    }

    pub fn approve_peer_with_response_and_permissions(
        &mut self,
        peer: SocketAddr,
        device_id: impl Into<String>,
        permissions: TrustedDevicePermissions,
    ) -> Result<Vec<TransportPacket>, BackendError> {
        let device_id = device_id.into();
        self.sessions
            .approve_with_permissions(peer, device_id.clone(), permissions);
        Ok(vec![
            self.build_pairing_result(true, Some(device_id), None)?,
            self.begin_session_key_exchange(peer)?,
        ])
    }

    pub fn challenge_peer_with_response(
        &mut self,
        peer: SocketAddr,
        expected_device_id: impl Into<String>,
    ) -> Result<(u64, TransportPacket), BackendError> {
        let expected_device_id = expected_device_id.into();
        let session = self.sessions.ensure_pending(peer);
        let public_key_der = session.device_public_key_der.clone().ok_or_else(|| {
            BackendError::Protocol(
                "trusted authentication requires a pairing public key".to_string(),
            )
        })?;

        let challenge = new_auth_challenge();
        session.pending_auth_challenge = Some(PendingAuthChallenge {
            challenge_id: challenge.challenge_id,
            nonce: challenge.nonce,
            issued_at_unix_ms: challenge.issued_at_unix_ms,
            expected_device_id,
            public_key_der,
        });

        let response = Message::AuthChallenge(challenge.clone());
        let payload = encode_frame(self.next_outbound_sequence, &response)
            .map_err(|err| BackendError::Protocol(err.to_string()))?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence += 1;
        Ok((
            challenge.challenge_id,
            TransportPacket {
                sequence,
                channel: ChannelKind::Control,
                message_kind: MessageKind::AuthChallenge,
                enqueue_timestamp_us: now_us(),
                payload,
            },
        ))
    }

    pub fn pairing_code_challenge_with_response(
        &mut self,
        peer: SocketAddr,
    ) -> Result<(String, u64, TransportPacket), BackendError> {
        let now = now_ms();
        if now >= self.pairing_code_expires_at_unix_ms {
            self.rotate_pairing_code();
        }
        let session = self.sessions.ensure_pending(peer);
        refresh_pairing_attempt_window(session, now);
        if session.pairing_code_attempts >= MAX_PAIRING_CODE_ATTEMPTS {
            return Err(BackendError::Protocol(
                "pairing code attempt limit reached".to_string(),
            ));
        }
        let mut salt = [0_u8; 32];
        OsRng.fill_bytes(&mut salt);
        let expires_at_unix_ms = now
            .saturating_add(PAIRING_CHALLENGE_TTL_MS)
            .min(self.pairing_code_expires_at_unix_ms);
        session.pending_pairing_challenge = Some(PendingPairingChallenge {
            salt,
            expires_at_unix_ms,
        });
        session.pairing_code_verified = false;

        let message = Message::PairingChallenge(PairingChallenge {
            salt,
            expires_at_unix_ms,
            code_digits: 6,
        });
        let payload = encode_frame(self.next_outbound_sequence, &message)
            .map_err(|error| BackendError::Protocol(error.to_string()))?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence = self.next_outbound_sequence.saturating_add(1);
        Ok((
            self.pairing_code.display().to_string(),
            expires_at_unix_ms,
            TransportPacket {
                sequence,
                channel: ChannelKind::Control,
                message_kind: MessageKind::PairingChallenge,
                enqueue_timestamp_us: now_us(),
                payload,
            },
        ))
    }

    fn verify_pairing_code_for_peer(
        &mut self,
        peer: SocketAddr,
        proof: &[u8],
    ) -> Result<(), String> {
        let now = now_ms();
        let session = self.sessions.ensure_pending(peer);
        refresh_pairing_attempt_window(session, now);
        if session.pairing_code_attempts >= MAX_PAIRING_CODE_ATTEMPTS {
            return Err("pairing code attempt limit reached".to_string());
        }
        session.pairing_code_verified = false;
        let Some(challenge) = session.pending_pairing_challenge.take() else {
            session.pairing_code_attempts = session.pairing_code_attempts.saturating_add(1);
            return Err("pairing code challenge was not requested".to_string());
        };
        if now > challenge.expires_at_unix_ms || now > self.pairing_code_expires_at_unix_ms {
            session.pairing_code_attempts = session.pairing_code_attempts.saturating_add(1);
            return Err("pairing code expired".to_string());
        }
        if verify_pairing_code_proof(&self.pairing_code, &challenge.salt, proof).is_err() {
            session.pairing_code_attempts = session.pairing_code_attempts.saturating_add(1);
            return Err("pairing code did not match".to_string());
        }

        session.pairing_code_attempts = 0;
        session.pairing_code_verified = true;
        self.rotate_pairing_code();
        Ok(())
    }

    fn rotate_pairing_code(&mut self) {
        self.pairing_code = PairingCode::generate();
        self.pairing_code_expires_at_unix_ms = now_ms().saturating_add(PAIRING_CODE_TTL_MS);
        for session in self.sessions.sessions.values_mut() {
            session.pending_pairing_challenge = None;
        }
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

    fn begin_session_key_exchange(
        &mut self,
        peer: SocketAddr,
    ) -> Result<TransportPacket, BackendError> {
        let ephemeral_secret = EphemeralSecret::random(&mut OsRng);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);
        let ephemeral_public_key_der = ephemeral_public
            .to_public_key_der()
            .map_err(|error| BackendError::Protocol(error.to_string()))?
            .as_bytes()
            .to_vec();
        let mut session_id = [0_u8; 16];
        let mut salt = [0_u8; 32];
        OsRng.fill_bytes(&mut session_id);
        OsRng.fill_bytes(&mut salt);
        let mut exchange = ServerKeyExchange {
            session_id,
            expires_at_unix_ms: now_ms().saturating_add(SESSION_KEY_EXCHANGE_TTL_MS),
            salt,
            ephemeral_public_key_der,
            host_identity_public_key_der: self
                .host_identity
                .public_key_der()
                .map_err(|error| BackendError::Protocol(error.to_string()))?,
            signature: Vec::new(),
        };
        exchange.signature = self
            .host_identity
            .sign_der(&server_signing_payload(&exchange));
        self.sessions.ensure_pending(peer).pending_key_exchange = Some(PendingKeyExchange {
            exchange: exchange.clone(),
            ephemeral_secret,
        });
        let payload = encode_server_key_exchange(&exchange)?;
        let sequence = self.next_outbound_sequence;
        self.next_outbound_sequence += 1;
        Ok(TransportPacket {
            sequence,
            channel: ChannelKind::Control,
            message_kind: MessageKind::SessionKeyExchange,
            enqueue_timestamp_us: now_us(),
            payload,
        })
    }

    fn finish_session_key_exchange(
        &mut self,
        peer: SocketAddr,
        payload: &[u8],
    ) -> Result<[u8; 16], BackendError> {
        let confirm = decode_client_key_confirm(payload)?;
        let session = self.sessions.ensure_pending(peer);
        let pending = session.pending_key_exchange.take().ok_or_else(|| {
            BackendError::Protocol("no session key exchange is pending".to_string())
        })?;
        if confirm.session_id != pending.exchange.session_id {
            return Err(BackendError::Protocol(
                "session key confirmation id did not match".to_string(),
            ));
        }
        if now_ms() > pending.exchange.expires_at_unix_ms {
            return Err(BackendError::Protocol(
                "session key exchange expired".to_string(),
            ));
        }
        if session.device_id.as_deref() != Some(confirm.device_id.as_str()) {
            return Err(BackendError::Protocol(
                "session key confirmation device id did not match".to_string(),
            ));
        }
        let identity_der = session.device_public_key_der.as_deref().ok_or_else(|| {
            BackendError::Protocol("session key confirmation requires a device key".to_string())
        })?;
        verify_client_key_confirm(identity_der, &pending.exchange, &confirm)?;

        let client_ephemeral = PublicKey::from_public_key_der(&confirm.ephemeral_public_key_der)
            .map_err(|error| BackendError::Protocol(error.to_string()))?;
        let shared = pending.ephemeral_secret.diffie_hellman(&client_ephemeral);
        let transcript = session_transcript_hash(&pending.exchange, &confirm);
        let secret = SecretBytes::from_bytes(shared.raw_secret_bytes().to_vec());
        let session_id = pending.exchange.session_id;
        session.secure_session = Some(ActiveSecureSession {
            session_id,
            ciphers: SessionCipherPair::for_host(&secret, &transcript),
            codec: SecureDatagramCodec::new(session_aad(&session_id)),
        });
        Ok(session_id)
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
            let pairing_code_proof = request.pairing_code_hash;
            let public_key_der = if request.one_time_public_key.is_empty() {
                None
            } else {
                Some(request.one_time_public_key)
            };
            let public_key_fingerprint = public_key_der.as_deref().and_then(public_key_fingerprint);
            session.device_id = Some(request.device_name.clone());
            session.device_public_key_der = public_key_der;
            session.device_public_key_fingerprint = public_key_fingerprint.clone();
            session.pending_auth_challenge = None;
            if self.permission_policy == PermissionPolicy::DevAutoApprove {
                outcome.events.push(BackendEvent::PairingRequested {
                    peer,
                    device_name: request.device_name,
                    public_key_fingerprint: public_key_fingerprint.clone(),
                    code_verified: true,
                });
                let device_id = public_key_fingerprint
                    .as_deref()
                    .map(trusted_device_id_from_public_key_fingerprint)
                    .unwrap_or_else(|| trusted_device_id(peer));
                session.permission = PermissionState::Approved;
                session.device_id = Some(device_id.clone());
                let response = self.build_pairing_result(true, Some(device_id), None)?;
                outcome.outbound.push((peer, response));
                let key_exchange = self.begin_session_key_exchange(peer)?;
                let session_id = self
                    .sessions
                    .ensure_pending(peer)
                    .pending_key_exchange
                    .as_ref()
                    .expect("exchange inserted")
                    .exchange
                    .session_id;
                outcome.outbound.push((peer, key_exchange));
                outcome.events.push(BackendEvent::PeerAutoApproved { peer });
                outcome
                    .events
                    .push(BackendEvent::SessionKeyExchangeQueued { peer, session_id });
                outcome.events.push(BackendEvent::PairingResultQueued {
                    peer,
                    accepted: true,
                });
                return Ok(outcome);
            }

            if pairing_code_proof.is_empty() {
                outcome.events.push(BackendEvent::PairingRequested {
                    peer,
                    device_name: request.device_name,
                    public_key_fingerprint,
                    code_verified: false,
                });
                return Ok(outcome);
            }

            match self.verify_pairing_code_for_peer(peer, &pairing_code_proof) {
                Ok(()) => outcome.events.push(BackendEvent::PairingRequested {
                    peer,
                    device_name: request.device_name,
                    public_key_fingerprint,
                    code_verified: true,
                }),
                Err(reason) => {
                    let attempts = self.sessions.ensure_pending(peer).pairing_code_attempts;
                    let attempts_remaining = MAX_PAIRING_CODE_ATTEMPTS.saturating_sub(attempts);
                    let response = self.build_pairing_result(false, None, Some(reason.clone()))?;
                    outcome.outbound.push((peer, response));
                    outcome.events.push(BackendEvent::PairingCodeRejected {
                        peer,
                        attempts_remaining,
                        reason,
                    });
                    outcome.events.push(BackendEvent::PairingResultQueued {
                        peer,
                        accepted: false,
                    });
                }
            }
            return Ok(outcome);
        }

        if packet.message_kind == MessageKind::AuthResponse {
            let frame = decode_frame(&packet.payload)
                .map_err(|err| BackendError::Protocol(err.to_string()))?;
            let Message::AuthResponse(response) = frame.message else {
                return Err(BackendError::Protocol(
                    "auth payload did not contain AuthResponse".to_string(),
                ));
            };
            let auth_result = verify_pending_auth_response(session, &response);
            match auth_result {
                Ok(trusted_device_id) => {
                    session.permission = PermissionState::Approved;
                    session.device_id = Some(trusted_device_id.clone());
                    session.pending_auth_challenge = None;
                    let response =
                        self.build_pairing_result(true, Some(trusted_device_id.clone()), None)?;
                    outcome.outbound.push((peer, response));
                    let key_exchange = self.begin_session_key_exchange(peer)?;
                    let session_id = self
                        .sessions
                        .ensure_pending(peer)
                        .pending_key_exchange
                        .as_ref()
                        .expect("exchange inserted")
                        .exchange
                        .session_id;
                    outcome.outbound.push((peer, key_exchange));
                    outcome
                        .events
                        .push(BackendEvent::SessionKeyExchangeQueued { peer, session_id });
                    outcome
                        .events
                        .push(BackendEvent::TrustedDeviceAuthenticated {
                            peer,
                            trusted_device_id,
                        });
                    outcome.events.push(BackendEvent::PairingResultQueued {
                        peer,
                        accepted: true,
                    });
                }
                Err(reason) => {
                    session.permission = PermissionState::Rejected;
                    session.pending_auth_challenge = None;
                    let response = self.build_pairing_result(false, None, Some(reason.clone()))?;
                    outcome.outbound.push((peer, response));
                    outcome
                        .events
                        .push(BackendEvent::PacketIgnored { peer, reason });
                    outcome.events.push(BackendEvent::PairingResultQueued {
                        peer,
                        accepted: false,
                    });
                }
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

        if packet.message_kind == MessageKind::SessionKeyConfirm {
            let session_id = self.finish_session_key_exchange(peer, &packet.payload)?;
            outcome
                .events
                .push(BackendEvent::SessionSecured { peer, session_id });
            return Ok(outcome);
        }

        if session.permission != PermissionState::Approved {
            outcome
                .events
                .push(BackendEvent::PermissionRequired { peer });
            return Ok(outcome);
        }

        if !self.sessions.allows_input(peer, packet.message_kind) {
            outcome.events.push(BackendEvent::PacketIgnored {
                peer,
                reason: format!(
                    "{:?} denied by trusted-device permissions",
                    packet.message_kind
                ),
            });
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
                if let Some(bridge) = self.gamepad_bridge.as_mut() {
                    bridge.inject_remote_gamepad(&gamepad)?;
                    outcome.events.push(BackendEvent::GamepadInjected {
                        peer,
                        controller_id: gamepad.controller_id,
                        connected: gamepad.connected,
                    });
                }
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
        Self::new_with_input_bridges(
            config,
            input_bridge,
            None,
            None,
            None,
            None,
            permission_policy,
        )
    }

    pub fn new_with_input_bridges(
        config: HostConfig,
        input_bridge: Option<StylusInputBridge<I>>,
        keyboard_bridge: Option<KeyboardInputBridge<Box<dyn KeyboardInjector>>>,
        touch_bridge: Option<TouchInputBridge<Box<dyn TouchInjector>>>,
        mouse_bridge: Option<MouseInputBridge<Box<dyn MouseInjector>>>,
        gamepad_bridge: Option<GamepadInputBridge<Box<dyn GamepadInjector>>>,
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
                gamepad_bridge,
                permission_policy,
            ),
            outbound: OutboundPacketQueues::default(),
            metrics: BackendMetrics::default(),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn set_host_identity(&mut self, identity: HostIdentity) {
        self.router.set_host_identity(identity);
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
        self.approve_peer_as_and_notify_with_permissions(
            server,
            peer,
            device_id,
            TrustedDevicePermissions::default(),
        )
    }

    pub fn approve_peer_as_and_notify_with_permissions(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
        device_id: impl Into<String>,
        permissions: TrustedDevicePermissions,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let device_id = device_id.into();
        let responses =
            self.router
                .approve_peer_with_response_and_permissions(peer, device_id, permissions)?;
        let key_exchange = responses
            .iter()
            .find(|packet| packet.message_kind == MessageKind::SessionKeyExchange)
            .ok_or_else(|| BackendError::Protocol("missing session key exchange".to_string()))?;
        let session_id =
            glyphray_protocol::session_wire::decode_server_key_exchange(&key_exchange.payload)?
                .session_id;
        for response in responses {
            server.send_to(&response, peer)?;
        }
        Ok(vec![
            BackendEvent::PeerApproved { peer },
            BackendEvent::PairingResultQueued {
                peer,
                accepted: true,
            },
            BackendEvent::SessionKeyExchangeQueued { peer, session_id },
        ])
    }

    pub fn challenge_peer_as_and_notify(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
        expected_device_id: impl Into<String>,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        self.challenge_peer_as_and_notify_with_permissions(
            server,
            peer,
            expected_device_id,
            TrustedDevicePermissions::default(),
        )
    }

    pub fn challenge_peer_as_and_notify_with_permissions(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
        expected_device_id: impl Into<String>,
        permissions: TrustedDevicePermissions,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        self.router.sessions.ensure_pending(peer).input_permissions = permissions;
        let (challenge_id, response) = self
            .router
            .challenge_peer_with_response(peer, expected_device_id)?;
        server.send_to(&response, peer)?;
        Ok(vec![BackendEvent::AuthChallengeQueued {
            peer,
            challenge_id,
        }])
    }

    pub fn issue_pairing_code_challenge(
        &mut self,
        server: &mut UdpServer,
        peer: SocketAddr,
    ) -> Result<Vec<BackendEvent>, BackendError> {
        let (display_code, expires_at_unix_ms, response) =
            self.router.pairing_code_challenge_with_response(peer)?;
        server.send_to(&response, peer)?;
        Ok(vec![BackendEvent::PairingCodeChallengeQueued {
            peer,
            display_code,
            expires_at_unix_ms,
        }])
    }

    pub fn is_pairing_code_verified(&self, peer: SocketAddr) -> bool {
        self.router
            .sessions
            .sessions
            .get(&peer)
            .is_some_and(|session| session.pairing_code_verified)
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
        let Some((datagram, peer)) = server.poll_recv_datagram()? else {
            return Ok(events);
        };
        self.metrics.received_packets += 1;

        let packet = match datagram {
            ReceivedDatagram::Plain(packet) => {
                if self.router.is_secure(peer) {
                    events.push(BackendEvent::PacketIgnored {
                        peer,
                        reason: "plaintext packet rejected after secure session establishment"
                            .to_string(),
                    });
                    return Ok(events);
                }
                if self.router.sessions.requires_secure_transport(peer)
                    && !matches!(
                        packet.message_kind,
                        MessageKind::PairingRequest
                            | MessageKind::AuthResponse
                            | MessageKind::SessionKeyConfirm
                    )
                {
                    events.push(BackendEvent::PacketIgnored {
                        peer,
                        reason: "plaintext packet rejected while secure session is pending"
                            .to_string(),
                    });
                    return Ok(events);
                }
                packet
            }
            ReceivedDatagram::Secure(sealed) => {
                match self.router.open_secure_packet(peer, &sealed) {
                    Ok(packet) => packet,
                    Err(_) => {
                        events.push(BackendEvent::PacketIgnored {
                            peer,
                            reason: "secure datagram authentication or replay check failed"
                                .to_string(),
                        });
                        return Ok(events);
                    }
                }
            }
        };

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

    pub fn update_device_permissions(
        &mut self,
        device_id: &str,
        permissions: TrustedDevicePermissions,
    ) -> usize {
        self.router
            .sessions
            .update_permissions_for_device(device_id, permissions)
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
            .next_back()
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

        let peers = self.router.sessions.secure_peers();
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

    pub fn queue_audio_packets_for_approved_peers(
        &mut self,
        packets: Vec<TransportPacket>,
    ) -> Vec<BackendEvent> {
        if packets.is_empty() {
            return Vec::new();
        }

        let peers = self.router.sessions.secure_peers();
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
        self.metrics.queued_audio_packets += queued as u64;
        vec![BackendEvent::AudioFrameQueued {
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
            let sent = if self.router.is_secure(peer) {
                let sealed = self.router.seal_secure_packet(peer, &packet)?;
                server.try_send_secure_to(&sealed, peer)?
            } else {
                server.try_send_to(&packet, peer)?
            };
            if sent {
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn refresh_pairing_attempt_window(session: &mut ClientSession, now_ms: u64) {
    if now_ms.saturating_sub(session.pairing_attempt_window_started_unix_ms)
        >= PAIRING_ATTEMPT_WINDOW_MS
    {
        session.pairing_code_attempts = 0;
        session.pairing_attempt_window_started_unix_ms = now_ms;
    }
}

fn new_auth_challenge() -> AuthChallenge {
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);
    AuthChallenge {
        challenge_id: OsRng.next_u64(),
        nonce,
        issued_at_unix_ms: now_ms(),
    }
}

fn verify_pending_auth_response(
    session: &ClientSession,
    response: &AuthResponse,
) -> Result<String, String> {
    let Some(challenge) = session.pending_auth_challenge.as_ref() else {
        return Err("auth response arrived without a pending challenge".to_string());
    };
    if response.challenge_id != challenge.challenge_id {
        return Err("auth response challenge id did not match".to_string());
    }
    if response.device_id != challenge.expected_device_id {
        return Err("auth response device id did not match the trusted device".to_string());
    }
    if now_ms().saturating_sub(challenge.issued_at_unix_ms) > TRUSTED_AUTH_CHALLENGE_TTL_MS {
        return Err("auth challenge expired".to_string());
    }

    let verifying_key = VerifyingKey::from_public_key_der(&challenge.public_key_der)
        .map_err(|error| format!("trusted device public key was invalid: {error}"))?;
    let signature = Signature::from_der(&response.signature)
        .map_err(|error| format!("auth response signature was invalid: {error}"))?;
    let payload = trusted_auth_challenge_payload(
        &challenge.expected_device_id,
        challenge.challenge_id,
        &challenge.nonce,
    );
    verifying_key
        .verify(&payload, &signature)
        .map_err(|_| "trusted device signature verification failed".to_string())?;
    Ok(challenge.expected_device_id.clone())
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
    events
        .iter()
        .any(|event| matches!(event, BackendEvent::SessionSecured { .. }))
}

fn verify_client_key_confirm(
    identity_public_key_der: &[u8],
    exchange: &ServerKeyExchange,
    confirm: &ClientKeyConfirm,
) -> Result<(), BackendError> {
    let verifying_key = VerifyingKey::from_public_key_der(identity_public_key_der)
        .map_err(|error| BackendError::Protocol(error.to_string()))?;
    let signature = Signature::from_der(&confirm.signature)
        .map_err(|error| BackendError::Protocol(error.to_string()))?;
    let server_hash = glyphray_protocol::session_wire::server_transcript_hash(exchange);
    verifying_key
        .verify(&client_signing_payload(&server_hash, confirm), &signature)
        .map_err(|_| BackendError::Protocol("client session signature did not verify".to_string()))
}

fn session_aad(session_id: &[u8; 16]) -> Vec<u8> {
    let mut aad = b"GlyphRay secure datagram v1".to_vec();
    aad.extend_from_slice(session_id);
    aad
}

fn current_displays() -> Vec<DisplayDescriptor> {
    WindowsGraphicsCaptureBackend::new()
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
        GamepadInjectionReport, GamepadInjector, KeyboardInjectionReport, KeyboardInjector,
        MouseInjectionReport, MouseInjector, PenInjector, TouchInjectionReport, TouchInjector,
    };
    use glyphray_core::{CoordinateMapper, DisplayRect, MappingMode, PressureMapper, SourceRect};
    use glyphray_protocol::session_wire::{
        decode_server_key_exchange, encode_client_key_confirm, server_transcript_hash,
    };
    use glyphray_protocol::stylus_wire::encode_stylus_batch;
    use glyphray_protocol::{
        AuthResponse, ColorSpace, EncoderConfig, GamepadInput, KeyboardInput, MouseInput,
        PairingRequest, StylusAction, StylusInputBatch, StylusSample, StylusToolType, TouchAction,
        TouchInputBatch, TouchSample, VideoCodec,
    };
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::SigningKey;

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

    #[derive(Default)]
    struct RecordingGamepadInjector;

    impl GamepadInjector for RecordingGamepadInjector {
        fn inject_gamepad(
            &mut self,
            input: &GamepadInput,
        ) -> Result<GamepadInjectionReport, InputError> {
            Ok(GamepadInjectionReport {
                updated_controllers: usize::from(input.connected),
                disconnected_controllers: usize::from(!input.connected),
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
            pairing_code_hash: vec![],
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
            code_verified: false,
        }));
    }

    #[test]
    fn one_time_pairing_code_unlocks_manual_approval_once() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50022".parse().expect("peer");
        let (display_code, _, challenge_packet) = router
            .pairing_code_challenge_with_response(peer)
            .expect("challenge");
        let challenge_frame = decode_frame(&challenge_packet.payload).expect("decode challenge");
        let Message::PairingChallenge(challenge) = challenge_frame.message else {
            panic!("expected pairing challenge");
        };
        let proof =
            glyphray_security::pairing_code_proof(&display_code, &challenge.salt).expect("proof");
        let message = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: proof.clone(),
            one_time_public_key: vec![4, 5, 6],
        });
        let packet = TransportPacket {
            sequence: 2,
            channel: ChannelKind::Control,
            message_kind: MessageKind::PairingRequest,
            enqueue_timestamp_us: 0,
            payload: encode_frame(2, &message).expect("encode"),
        };

        let outcome = router.route_packet(peer, packet).expect("route proof");
        assert!(outcome.events.iter().any(|event| matches!(
            event,
            BackendEvent::PairingRequested {
                peer: event_peer,
                code_verified: true,
                ..
            } if *event_peer == peer
        )));
        assert!(
            router
                .sessions
                .sessions
                .get(&peer)
                .expect("session")
                .pairing_code_verified
        );

        let replay = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: proof,
            one_time_public_key: vec![4, 5, 6],
        });
        let replay_outcome = router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 3,
                    channel: ChannelKind::Control,
                    message_kind: MessageKind::PairingRequest,
                    enqueue_timestamp_us: 0,
                    payload: encode_frame(3, &replay).expect("encode replay"),
                },
            )
            .expect("route replay");
        assert!(replay_outcome.events.iter().any(|event| matches!(
            event,
            BackendEvent::PairingCodeRejected { peer: event_peer, .. } if *event_peer == peer
        )));
    }

    #[test]
    fn trusted_auth_response_approves_pending_peer() {
        use p256::ecdsa::signature::Signer;
        use p256::ecdsa::{Signature, SigningKey};
        use p256::elliptic_curve::rand_core::OsRng as P256OsRng;
        use p256::pkcs8::EncodePublicKey;

        let signing_key = SigningKey::random(&mut P256OsRng);
        let public_key_der = signing_key
            .verifying_key()
            .to_public_key_der()
            .expect("public key DER")
            .as_bytes()
            .to_vec();
        let fingerprint = public_key_fingerprint(&public_key_der).expect("fingerprint");
        let device_id = trusted_device_id_from_public_key_fingerprint(&fingerprint);
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50011".parse().expect("peer");
        let message = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: vec![],
            one_time_public_key: public_key_der,
        });
        let packet = TransportPacket {
            sequence: 1,
            channel: ChannelKind::Control,
            message_kind: MessageKind::PairingRequest,
            enqueue_timestamp_us: 0,
            payload: encode_frame(1, &message).expect("encode"),
        };
        router.route_packet(peer, packet).expect("pairing");
        let (_, challenge_packet) = router
            .challenge_peer_with_response(peer, device_id.clone())
            .expect("challenge");
        let challenge_frame = decode_frame(&challenge_packet.payload).expect("challenge frame");
        let Message::AuthChallenge(challenge) = challenge_frame.message else {
            panic!("expected AuthChallenge");
        };
        let payload =
            trusted_auth_challenge_payload(&device_id, challenge.challenge_id, &challenge.nonce);
        let signature: Signature = signing_key.sign(&payload);
        let auth_response = Message::AuthResponse(AuthResponse {
            challenge_id: challenge.challenge_id,
            device_id: device_id.clone(),
            signature: signature.to_der().as_bytes().to_vec(),
        });
        let auth_packet = TransportPacket {
            sequence: 2,
            channel: ChannelKind::Control,
            message_kind: MessageKind::AuthResponse,
            enqueue_timestamp_us: 0,
            payload: encode_frame(2, &auth_response).expect("auth response"),
        };

        let outcome = router.route_packet(peer, auth_packet).expect("auth");

        assert!(outcome
            .events
            .contains(&BackendEvent::TrustedDeviceAuthenticated {
                peer,
                trusted_device_id: device_id,
            }));
        assert!(outcome.events.contains(&BackendEvent::PairingResultQueued {
            peer,
            accepted: true,
        }));
        assert!(router.sessions.is_approved(peer));
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
        assert_eq!(outcome.outbound.len(), 2);
        assert_eq!(
            outcome.outbound[0].1.message_kind,
            MessageKind::PairingResult
        );
        assert_eq!(
            outcome.outbound[1].1.message_kind,
            MessageKind::SessionKeyExchange
        );
    }

    #[test]
    fn session_key_handshake_authenticates_and_opens_client_datagram() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50014".parse().expect("peer");
        let (mut client_codec, client_ciphers) = complete_secure_handshake(&mut router, peer);
        let packet = TransportPacket {
            sequence: 91,
            channel: ChannelKind::Control,
            message_kind: MessageKind::LatencyPing,
            enqueue_timestamp_us: 100,
            payload: vec![1, 2, 3],
        };
        let encoded = encode_packet(&packet).expect("encode transport packet");
        let sealed = client_codec
            .seal(&client_ciphers.outbound, &encoded)
            .expect("seal client datagram");

        assert_eq!(
            router
                .open_secure_packet(peer, &sealed)
                .expect("open client datagram"),
            packet
        );
        assert!(router.is_secure(peer));
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
    fn trusted_device_permissions_block_denied_input_before_decode() {
        let mut router = HostPacketRouter::<RecordingInjector>::new(None);
        let peer: SocketAddr = "127.0.0.1:50016".parse().expect("peer");
        let permissions = TrustedDevicePermissions {
            allow_keyboard: false,
            ..TrustedDevicePermissions::default()
        };
        router.approve_peer_with_permissions(peer, "tablet", permissions);
        let keyboard = Message::KeyboardInput(KeyboardInput {
            sequence: 1,
            timestamp_us: 200,
            scan_code: 30,
            virtual_key: 65,
            pressed: true,
            modifiers: 0,
        });

        let outcome = router
            .route_packet(
                peer,
                framed_input_packet(MessageKind::KeyboardInput, keyboard),
            )
            .expect("route denied keyboard");

        assert!(outcome.events.contains(&BackendEvent::PacketIgnored {
            peer,
            reason: "KeyboardInput denied by trusted-device permissions".to_string(),
        }));
        assert!(!outcome
            .events
            .iter()
            .any(|event| matches!(event, BackendEvent::KeyboardDecoded { .. })));
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
            None,
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
    fn approved_peer_gamepad_packet_can_be_injected() {
        let gamepad_bridge =
            GamepadInputBridge::new(Box::new(RecordingGamepadInjector) as Box<dyn GamepadInjector>);
        let mut router = HostPacketRouter::<RecordingInjector>::new_with_input_bridges(
            None,
            None,
            None,
            None,
            Some(gamepad_bridge),
            PermissionPolicy::RequireApproval,
        );
        let peer: SocketAddr = "127.0.0.1:50011".parse().expect("peer");
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

        assert!(outcome.events.contains(&BackendEvent::GamepadInjected {
            peer,
            controller_id: 7,
            connected: true,
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
    fn runtime_queues_video_packets_for_secure_peers_only() {
        let mut runtime = HostBackendRuntime::<RecordingInjector>::new(HostConfig::default(), None);
        let approved: SocketAddr = "127.0.0.1:53004".parse().expect("peer");
        let pending: SocketAddr = "127.0.0.1:53005".parse().expect("peer");
        complete_secure_handshake(&mut runtime.router, approved);
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
    fn runtime_queues_audio_packets_for_secure_peers_only() {
        let mut runtime = HostBackendRuntime::<RecordingInjector>::new(HostConfig::default(), None);
        let approved: SocketAddr = "127.0.0.1:53006".parse().expect("peer");
        let pending: SocketAddr = "127.0.0.1:53007".parse().expect("peer");
        complete_secure_handshake(&mut runtime.router, approved);
        runtime
            .router
            .route_packet(pending, input_packet(vec![1, 2, 3]))
            .expect("route");

        let events = runtime.queue_audio_packets_for_approved_peers(vec![packet_with_channel(
            12,
            ChannelKind::Audio,
        )]);
        let snapshot = runtime.health_snapshot();

        assert_eq!(
            events,
            vec![BackendEvent::AudioFrameQueued {
                peers: 1,
                packets: 1,
            }]
        );
        assert_eq!(snapshot.outbound.audio, 1);
        assert_eq!(snapshot.outbound.video, 0);
        assert_eq!(snapshot.metrics.queued_audio_packets, 1);
    }

    fn complete_secure_handshake(
        router: &mut HostPacketRouter<RecordingInjector>,
        peer: SocketAddr,
    ) -> (SecureDatagramCodec, SessionCipherPair) {
        let device_identity = SigningKey::random(&mut OsRng);
        let device_public_key_der = device_identity
            .verifying_key()
            .to_public_key_der()
            .expect("device public key DER")
            .as_bytes()
            .to_vec();
        let fingerprint = public_key_fingerprint(&device_public_key_der).expect("fingerprint");
        let device_id = trusted_device_id_from_public_key_fingerprint(&fingerprint);
        let pairing = Message::PairingRequest(PairingRequest {
            device_name: "Galaxy Tab".to_string(),
            pairing_code_hash: vec![],
            one_time_public_key: device_public_key_der,
        });
        router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 1,
                    channel: ChannelKind::Control,
                    message_kind: MessageKind::PairingRequest,
                    enqueue_timestamp_us: 0,
                    payload: encode_frame(1, &pairing).expect("pairing frame"),
                },
            )
            .expect("pairing request");
        let responses = router
            .approve_peer_with_response(peer, device_id.clone())
            .expect("approve peer");
        let exchange = responses
            .iter()
            .find(|packet| packet.message_kind == MessageKind::SessionKeyExchange)
            .map(|packet| decode_server_key_exchange(&packet.payload).expect("server exchange"))
            .expect("key exchange response");

        let client_secret = EphemeralSecret::random(&mut OsRng);
        let client_public = PublicKey::from(&client_secret)
            .to_public_key_der()
            .expect("client ephemeral DER")
            .as_bytes()
            .to_vec();
        let mut confirm = ClientKeyConfirm {
            session_id: exchange.session_id,
            device_id,
            ephemeral_public_key_der: client_public,
            signature: Vec::new(),
        };
        let server_hash = server_transcript_hash(&exchange);
        let signature: p256::ecdsa::Signature =
            device_identity.sign(&client_signing_payload(&server_hash, &confirm));
        confirm.signature = signature.to_der().as_bytes().to_vec();

        let host_ephemeral = PublicKey::from_public_key_der(&exchange.ephemeral_public_key_der)
            .expect("host ephemeral public key");
        let shared = client_secret.diffie_hellman(&host_ephemeral);
        let transcript = session_transcript_hash(&exchange, &confirm);
        let client_ciphers = SessionCipherPair::for_client(
            &SecretBytes::from_bytes(shared.raw_secret_bytes().to_vec()),
            &transcript,
        );
        let outcome = router
            .route_packet(
                peer,
                TransportPacket {
                    sequence: 2,
                    channel: ChannelKind::Control,
                    message_kind: MessageKind::SessionKeyConfirm,
                    enqueue_timestamp_us: 0,
                    payload: encode_client_key_confirm(&confirm).expect("client confirm"),
                },
            )
            .expect("complete secure handshake");
        assert!(outcome
            .events
            .iter()
            .any(|event| matches!(event, BackendEvent::SessionSecured { .. })));

        (
            SecureDatagramCodec::new(session_aad(&exchange.session_id)),
            client_ciphers,
        )
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
