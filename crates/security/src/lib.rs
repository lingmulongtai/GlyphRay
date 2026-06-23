use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;
use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha256>;
const PAIRING_PROOF_DOMAIN: &[u8] = b"GlyphRay pairing proof v1";

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("pairing code was invalid or expired")]
    InvalidPairingCode,
    #[error("too many pairing attempts")]
    RateLimited,
    #[error("secret store error: {0}")]
    SecretStore(String),
    #[error("authentication tag did not verify")]
    InvalidAuthenticationTag,
    #[error("session cipher operation failed")]
    Cipher,
    #[error("packet replay or stale counter detected")]
    Replay,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("DeviceId").field(&self.0).finish()
    }
}

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SecretBytes(Vec<u8>);

impl SecretBytes {
    pub fn random(len: usize) -> Self {
        let mut bytes = vec![0_u8; len];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    pub fn generate() -> Self {
        let value = OsRng.next_u32() % 1_000_000;
        Self(format!("{:03}-{:03}", value / 1000, value % 1000))
    }

    pub fn from_digits_for_test(value: u32) -> Self {
        let value = value % 1_000_000;
        Self(format!("{:03}-{:03}", value / 1000, value % 1000))
    }

    pub fn display(&self) -> &str {
        &self.0
    }

    pub fn hash_for_transport(&self, salt: &[u8]) -> Vec<u8> {
        self.proof(salt)
    }

    pub fn proof(&self, salt: &[u8]) -> Vec<u8> {
        pairing_code_proof(&self.0, salt).expect("generated pairing codes are valid")
    }
}

pub fn pairing_code_proof(code: &str, salt: &[u8]) -> Result<Vec<u8>, SecurityError> {
    let canonical = canonical_pairing_code(code).ok_or(SecurityError::InvalidPairingCode)?;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt).expect("HMAC accepts any key length");
    mac.update(PAIRING_PROOF_DOMAIN);
    mac.update(canonical.as_bytes());
    Ok(mac.finalize().into_bytes().to_vec())
}

pub fn verify_pairing_code_proof(
    code: &PairingCode,
    salt: &[u8],
    proof: &[u8],
) -> Result<(), SecurityError> {
    let canonical =
        canonical_pairing_code(code.display()).ok_or(SecurityError::InvalidPairingCode)?;
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt).expect("HMAC accepts any key length");
    mac.update(PAIRING_PROOF_DOMAIN);
    mac.update(canonical.as_bytes());
    mac.verify_slice(proof)
        .map_err(|_| SecurityError::InvalidPairingCode)
}

fn canonical_pairing_code(code: &str) -> Option<String> {
    let digits: String = code
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect();
    (digits.len() == 6).then_some(digits)
}

pub struct PairingRateLimiter {
    max_attempts: u32,
    window: Duration,
    attempts: HashMap<String, AttemptWindow>,
}

#[derive(Debug, Clone)]
struct AttemptWindow {
    started_at: Instant,
    count: u32,
}

impl PairingRateLimiter {
    pub fn new(max_attempts: u32, window: Duration) -> Self {
        Self {
            max_attempts,
            window,
            attempts: HashMap::new(),
        }
    }

    pub fn check(&mut self, remote_id: &str) -> Result<(), SecurityError> {
        let now = Instant::now();
        let entry = self
            .attempts
            .entry(remote_id.to_string())
            .or_insert(AttemptWindow {
                started_at: now,
                count: 0,
            });

        if now.duration_since(entry.started_at) > self.window {
            entry.started_at = now;
            entry.count = 0;
        }

        entry.count += 1;
        if entry.count > self.max_attempts {
            return Err(SecurityError::RateLimited);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AuthChallenge {
    pub id: u64,
    pub nonce: [u8; 32],
}

impl AuthChallenge {
    pub fn generate(id: u64) -> Self {
        let mut nonce = [0_u8; 32];
        OsRng.fill_bytes(&mut nonce);
        Self { id, nonce }
    }
}

pub fn sign_challenge(
    shared_secret: &SecretBytes,
    device_id: &DeviceId,
    challenge: &AuthChallenge,
) -> Vec<u8> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(shared_secret.expose())
        .expect("HMAC accepts any key length");
    mac.update(device_id.as_str().as_bytes());
    mac.update(&challenge.id.to_le_bytes());
    mac.update(&challenge.nonce);
    mac.finalize().into_bytes().to_vec()
}

pub fn verify_challenge(
    shared_secret: &SecretBytes,
    device_id: &DeviceId,
    challenge: &AuthChallenge,
    tag: &[u8],
) -> Result<(), SecurityError> {
    let expected = sign_challenge(shared_secret, device_id, challenge);
    let mut mac = <HmacSha256 as Mac>::new_from_slice(shared_secret.expose())
        .expect("HMAC accepts any key length");
    mac.update(device_id.as_str().as_bytes());
    mac.update(&challenge.id.to_le_bytes());
    mac.update(&challenge.nonce);
    mac.verify_slice(tag)
        .map_err(|_| SecurityError::InvalidAuthenticationTag)?;
    debug_assert_eq!(expected.len(), tag.len());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionToken {
    encoded: String,
    expires_at: Instant,
}

impl SessionToken {
    pub fn issue(ttl: Duration) -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self {
            encoded: URL_SAFE_NO_PAD.encode(bytes),
            expires_at: Instant::now() + ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    pub fn as_str(&self) -> &str {
        &self.encoded
    }
}

pub trait SecretStore {
    fn put_device_secret(
        &mut self,
        device_id: &DeviceId,
        secret: SecretBytes,
    ) -> Result<(), SecurityError>;

    fn get_device_secret(&self, device_id: &DeviceId)
        -> Result<Option<SecretBytes>, SecurityError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedPacket {
    pub counter: u64,
    pub ciphertext: Vec<u8>,
}

pub struct SessionCipher {
    cipher: Aes256Gcm,
}

impl SessionCipher {
    pub fn new(secret: &SecretBytes, transcript_hash: &[u8]) -> Self {
        Self::derive(secret, transcript_hash, b"single-direction")
    }

    pub fn derive(secret: &SecretBytes, transcript_hash: &[u8], direction: &[u8]) -> Self {
        let key = derive_session_key(secret.expose(), transcript_hash, direction);
        Self {
            cipher: Aes256Gcm::new_from_slice(&key).expect("32-byte key"),
        }
    }

    pub fn seal(
        &self,
        counter: u64,
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<SealedPacket, SecurityError> {
        let nonce = nonce_from_counter(counter);
        let ciphertext = self
            .cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| SecurityError::Cipher)?;
        Ok(SealedPacket {
            counter,
            ciphertext,
        })
    }

    pub fn open(&self, packet: &SealedPacket, aad: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let nonce = nonce_from_counter(packet.counter);
        self.cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &packet.ciphertext,
                    aad,
                },
            )
            .map_err(|_| SecurityError::InvalidAuthenticationTag)
    }
}

pub struct SessionCipherPair {
    pub outbound: SessionCipher,
    pub inbound: SessionCipher,
}

impl SessionCipherPair {
    pub fn for_host(secret: &SecretBytes, transcript_hash: &[u8]) -> Self {
        Self {
            outbound: SessionCipher::derive(secret, transcript_hash, b"host-to-client"),
            inbound: SessionCipher::derive(secret, transcript_hash, b"client-to-host"),
        }
    }

    pub fn for_client(secret: &SecretBytes, transcript_hash: &[u8]) -> Self {
        Self {
            outbound: SessionCipher::derive(secret, transcript_hash, b"client-to-host"),
            inbound: SessionCipher::derive(secret, transcript_hash, b"host-to-client"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayGuard {
    highest_counter: Option<u64>,
    window: u64,
    seen: BTreeSet<u64>,
}

impl ReplayGuard {
    pub fn new(window: u64) -> Self {
        Self {
            highest_counter: None,
            window: window.max(1),
            seen: BTreeSet::new(),
        }
    }

    pub fn accept(&mut self, counter: u64) -> Result<(), SecurityError> {
        if self.seen.contains(&counter) {
            return Err(SecurityError::Replay);
        }
        if self
            .highest_counter
            .is_some_and(|highest| counter.saturating_add(self.window) <= highest)
        {
            return Err(SecurityError::Replay);
        }

        self.highest_counter = Some(
            self.highest_counter
                .map_or(counter, |high| high.max(counter)),
        );
        self.seen.insert(counter);
        let oldest_allowed = self
            .highest_counter
            .expect("set above")
            .saturating_sub(self.window.saturating_sub(1));
        self.seen = self.seen.split_off(&oldest_allowed);
        Ok(())
    }
}

fn derive_session_key(shared_secret: &[u8], transcript_hash: &[u8], direction: &[u8]) -> [u8; 32] {
    let mut extract =
        <HmacSha256 as Mac>::new_from_slice(transcript_hash).expect("HMAC accepts any key length");
    extract.update(shared_secret);
    let prk = extract.finalize().into_bytes();

    let mut expand =
        <HmacSha256 as Mac>::new_from_slice(&prk).expect("HMAC accepts any key length");
    expand.update(b"GlyphRay session key v1");
    expand.update(direction);
    expand.update(&[1]);
    expand.finalize().into_bytes().into()
}

fn nonce_from_counter(counter: u64) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[0..4].copy_from_slice(b"GLYR");
    nonce[4..12].copy_from_slice(&counter.to_be_bytes());
    nonce
}

#[derive(Default)]
pub struct InMemorySecretStore {
    secrets: HashMap<DeviceId, Vec<u8>>,
}

impl SecretStore for InMemorySecretStore {
    fn put_device_secret(
        &mut self,
        device_id: &DeviceId,
        secret: SecretBytes,
    ) -> Result<(), SecurityError> {
        self.secrets
            .insert(device_id.clone(), secret.expose().to_vec());
        Ok(())
    }

    fn get_device_secret(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<SecretBytes>, SecurityError> {
        Ok(self
            .secrets
            .get(device_id)
            .map(|secret| SecretBytes::from_bytes(secret.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_lower(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    #[test]
    fn pairing_code_hash_depends_on_salt() {
        let code = PairingCode::from_digits_for_test(123456);
        assert_ne!(code.hash_for_transport(b"a"), code.hash_for_transport(b"b"));
    }

    #[test]
    fn challenge_response_verifies_with_same_secret() {
        let secret = SecretBytes::from_bytes(b"shared test secret".to_vec());
        let device = DeviceId::new("android-tablet-1");
        let challenge = AuthChallenge {
            id: 10,
            nonce: [7_u8; 32],
        };

        let tag = sign_challenge(&secret, &device, &challenge);
        verify_challenge(&secret, &device, &challenge, &tag).expect("verified");
    }

    #[test]
    fn challenge_response_rejects_wrong_device() {
        let secret = SecretBytes::from_bytes(b"shared test secret".to_vec());
        let challenge = AuthChallenge {
            id: 10,
            nonce: [7_u8; 32],
        };

        let tag = sign_challenge(&secret, &DeviceId::new("a"), &challenge);
        assert!(matches!(
            verify_challenge(&secret, &DeviceId::new("b"), &challenge, &tag),
            Err(SecurityError::InvalidAuthenticationTag)
        ));
    }

    #[test]
    fn pairing_attempts_are_rate_limited() {
        let mut limiter = PairingRateLimiter::new(2, Duration::from_secs(60));
        limiter.check("lan-peer").unwrap();
        limiter.check("lan-peer").unwrap();
        assert!(matches!(
            limiter.check("lan-peer"),
            Err(SecurityError::RateLimited)
        ));
    }

    #[test]
    fn session_cipher_round_trips_and_authenticates_aad() {
        let secret = SecretBytes::from_bytes(b"shared secret used for cipher".to_vec());
        let cipher = SessionCipher::new(&secret, b"handshake");
        let sealed = cipher.seal(1, b"video", b"payload").expect("seal");
        assert_eq!(cipher.open(&sealed, b"video").expect("open"), b"payload");
        assert!(matches!(
            cipher.open(&sealed, b"input"),
            Err(SecurityError::InvalidAuthenticationTag)
        ));
    }

    #[test]
    fn directional_session_keys_interoperate_without_nonce_reuse() {
        let secret = SecretBytes::from_bytes(b"shared ECDH secret".to_vec());
        let host = SessionCipherPair::for_host(&secret, b"transcript");
        let client = SessionCipherPair::for_client(&secret, b"transcript");

        let client_packet = client
            .outbound
            .seal(1, b"session", b"pen input")
            .expect("client seal");
        assert_eq!(
            host.inbound
                .open(&client_packet, b"session")
                .expect("host open"),
            b"pen input"
        );
        let host_packet = host
            .outbound
            .seal(1, b"session", b"video")
            .expect("host seal");
        assert_eq!(
            client
                .inbound
                .open(&host_packet, b"session")
                .expect("client open"),
            b"video"
        );
    }

    #[test]
    fn directional_key_derivation_matches_cross_platform_vector() {
        let shared_secret = (0_u8..32)
            .map(|value| value.wrapping_mul(7))
            .collect::<Vec<_>>();
        let transcript = (0_u8..32)
            .map(|value| value.wrapping_mul(11))
            .collect::<Vec<_>>();

        assert_eq!(
            hex_lower(&derive_session_key(
                &shared_secret,
                &transcript,
                b"client-to-host"
            )),
            "13a86c080847160ebf3331bdddd11ad8377be092698e6809c3af81fbf7c6dd0e"
        );
        assert_eq!(
            hex_lower(&derive_session_key(
                &shared_secret,
                &transcript,
                b"host-to-client"
            )),
            "f6daad80d2a79845aa4b0f67abac4ea0412a78ff2ffcdb029874375639bc498d"
        );
    }

    #[test]
    fn replay_guard_rejects_duplicate_counter() {
        let mut guard = ReplayGuard::new(64);
        guard.accept(10).expect("first");
        assert!(matches!(guard.accept(10), Err(SecurityError::Replay)));
        guard.accept(11).expect("next");
    }

    #[test]
    fn replay_guard_allows_unseen_out_of_order_packet_inside_window() {
        let mut guard = ReplayGuard::new(64);
        guard.accept(10).expect("first");
        guard.accept(12).expect("ahead");
        guard.accept(11).expect("reordered");
        assert!(matches!(guard.accept(11), Err(SecurityError::Replay)));
    }

    #[test]
    fn replay_guard_rejects_packet_older_than_window() {
        let mut guard = ReplayGuard::new(4);
        guard.accept(10).expect("first");
        assert!(matches!(guard.accept(6), Err(SecurityError::Replay)));
    }

    #[test]
    fn pairing_proof_accepts_formatted_or_plain_code_and_rejects_other_salt() {
        let code = PairingCode::from_digits_for_test(123456);
        let salt: [u8; 32] = std::array::from_fn(|index| index as u8);
        let proof = pairing_code_proof("123 456", &salt).expect("proof");
        assert_eq!(
            hex_lower(&proof),
            "f9b2e23be7a5543d2f02ce8063bf94df5c74485737dee573cc8bd3802d29d280"
        );
        verify_pairing_code_proof(&code, &salt, &proof).expect("verify");
        assert_eq!(proof, code.proof(&salt));
        assert!(matches!(
            verify_pairing_code_proof(&code, &[8_u8; 32], &proof),
            Err(SecurityError::InvalidPairingCode)
        ));
        assert!(matches!(
            pairing_code_proof("12345", &salt),
            Err(SecurityError::InvalidPairingCode)
        ));
    }
}
