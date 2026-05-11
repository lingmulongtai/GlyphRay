use glyphray_security::{
    DeviceId, InMemorySecretStore, PairingCode, PairingRateLimiter, SecretBytes, SecretStore,
    SecurityError,
};
use std::time::Duration;

pub struct PairingService<S: SecretStore> {
    secret_store: S,
    limiter: PairingRateLimiter,
}

impl Default for PairingService<InMemorySecretStore> {
    fn default() -> Self {
        Self::new(InMemorySecretStore::default())
    }
}

impl<S: SecretStore> PairingService<S> {
    pub fn new(secret_store: S) -> Self {
        Self {
            secret_store,
            limiter: PairingRateLimiter::new(5, Duration::from_secs(120)),
        }
    }

    pub fn issue_code(&self) -> PairingCode {
        PairingCode::generate()
    }

    pub fn trust_device(
        &mut self,
        remote_address: &str,
        device_id: DeviceId,
        shared_secret: SecretBytes,
    ) -> Result<(), SecurityError> {
        self.limiter.check(remote_address)?;
        self.secret_store
            .put_device_secret(&device_id, shared_secret)
    }
}

