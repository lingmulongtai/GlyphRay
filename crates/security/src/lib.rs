use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha256>;

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
        let mut hasher = Sha256::new();
        hasher.update(salt);
        hasher.update(self.0.as_bytes());
        hasher.finalize().to_vec()
    }
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
    let mut mac =
        HmacSha256::new_from_slice(shared_secret.expose()).expect("HMAC accepts any key length");
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
    let mut mac =
        HmacSha256::new_from_slice(shared_secret.expose()).expect("HMAC accepts any key length");
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

    fn get_device_secret(&self, device_id: &DeviceId) -> Result<Option<SecretBytes>, SecurityError>;
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

    fn get_device_secret(&self, device_id: &DeviceId) -> Result<Option<SecretBytes>, SecurityError> {
        Ok(self
            .secrets
            .get(device_id)
            .map(|secret| SecretBytes::from_bytes(secret.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

