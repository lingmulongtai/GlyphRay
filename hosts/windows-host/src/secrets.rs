use glyphray_security::{DeviceId, InMemorySecretStore, SecretBytes, SecretStore, SecurityError};

pub struct PlatformSecretStore {
    inner: InMemorySecretStore,
}

impl PlatformSecretStore {
    pub fn open() -> Result<Self, SecurityError> {
        Ok(Self {
            inner: InMemorySecretStore::default(),
        })
    }
}

impl SecretStore for PlatformSecretStore {
    fn put_device_secret(
        &mut self,
        device_id: &DeviceId,
        secret: SecretBytes,
    ) -> Result<(), SecurityError> {
        self.inner.put_device_secret(device_id, secret)
    }

    fn get_device_secret(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<SecretBytes>, SecurityError> {
        self.inner.get_device_secret(device_id)
    }
}
