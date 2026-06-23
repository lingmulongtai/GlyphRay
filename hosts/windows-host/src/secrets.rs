use crate::persistence::{atomic_write, quarantine_file};
use glyphray_security::{DeviceId, SecretBytes, SecretStore, SecurityError};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[cfg(not(windows))]
use glyphray_security::InMemorySecretStore;

#[cfg(windows)]
pub struct PlatformSecretStore {
    root: PathBuf,
}

#[derive(Clone)]
pub struct HostIdentity {
    signing_key: SigningKey,
}

pub struct HostIdentityLoad {
    pub identity: HostIdentity,
    pub quarantined_path: Option<PathBuf>,
}

impl HostIdentity {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::random(&mut OsRng),
        }
    }

    pub fn public_key_der(&self) -> Result<Vec<u8>, SecurityError> {
        self.signing_key
            .verifying_key()
            .to_public_key_der()
            .map(|document| document.as_bytes().to_vec())
            .map_err(secret_error)
    }

    pub fn fingerprint(&self) -> Result<String, SecurityError> {
        Ok(hex_lower(&Sha256::digest(self.public_key_der()?)))
    }

    pub fn sign_der(&self, payload: &[u8]) -> Vec<u8> {
        let signature: Signature = self.signing_key.sign(payload);
        signature.to_der().as_bytes().to_vec()
    }
}

impl std::fmt::Debug for HostIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HostIdentity")
            .field("fingerprint", &self.fingerprint().ok())
            .finish_non_exhaustive()
    }
}

pub fn load_or_create_host_identity(
    store: &mut PlatformSecretStore,
) -> Result<HostIdentity, SecurityError> {
    Ok(load_or_recover_host_identity(store)?.identity)
}

pub fn load_or_recover_host_identity(
    store: &mut PlatformSecretStore,
) -> Result<HostIdentityLoad, SecurityError> {
    let device_id = DeviceId::new("glyphray-host-identity-v1");
    match store.get_device_secret(&device_id) {
        Ok(Some(secret)) => {
            if let Ok(signing_key) = SigningKey::from_slice(secret.expose()) {
                return Ok(HostIdentityLoad {
                    identity: HostIdentity { signing_key },
                    quarantined_path: None,
                });
            }
        }
        Ok(None) => {}
        Err(_) => {
            let quarantined_path = store.quarantine_device_secret(&device_id)?;
            return create_recovered_identity(store, &device_id, quarantined_path);
        }
    }

    let quarantined_path = store.quarantine_device_secret(&device_id)?;
    create_recovered_identity(store, &device_id, quarantined_path)
}

fn create_recovered_identity(
    store: &mut PlatformSecretStore,
    device_id: &DeviceId,
    quarantined_path: Option<PathBuf>,
) -> Result<HostIdentityLoad, SecurityError> {
    let identity = HostIdentity::generate();
    store.put_device_secret(
        device_id,
        SecretBytes::from_bytes(identity.signing_key.to_bytes().to_vec()),
    )?;
    Ok(HostIdentityLoad {
        identity,
        quarantined_path,
    })
}

#[cfg(not(windows))]
pub struct PlatformSecretStore {
    inner: InMemorySecretStore,
}

#[cfg(windows)]
impl PlatformSecretStore {
    pub fn open() -> Result<Self, SecurityError> {
        Self::open_at(default_secret_dir()?)
    }

    pub fn open_at(root: impl Into<PathBuf>) -> Result<Self, SecurityError> {
        let root = root.into();
        std::fs::create_dir_all(&root).map_err(secret_error)?;
        Ok(Self { root })
    }

    fn path_for(&self, device_id: &DeviceId) -> PathBuf {
        self.root.join(format!(
            "device-{:08x}.dpapi",
            stable_device_id_hash(device_id)
        ))
    }

    fn quarantine_device_secret(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<PathBuf>, SecurityError> {
        quarantine_file(&self.path_for(device_id), "corrupt").map_err(secret_error)
    }
}

#[cfg(not(windows))]
impl PlatformSecretStore {
    pub fn open() -> Result<Self, SecurityError> {
        Ok(Self {
            inner: InMemorySecretStore::default(),
        })
    }

    pub fn open_at(_root: impl Into<PathBuf>) -> Result<Self, SecurityError> {
        Ok(Self {
            inner: InMemorySecretStore::default(),
        })
    }

    fn quarantine_device_secret(
        &self,
        _device_id: &DeviceId,
    ) -> Result<Option<PathBuf>, SecurityError> {
        Ok(None)
    }
}

#[cfg(windows)]
impl SecretStore for PlatformSecretStore {
    fn put_device_secret(
        &mut self,
        device_id: &DeviceId,
        secret: SecretBytes,
    ) -> Result<(), SecurityError> {
        let protected = dpapi::protect_secret(secret.expose())?;
        atomic_write(&self.path_for(device_id), &protected).map_err(secret_error)
    }

    fn get_device_secret(
        &self,
        device_id: &DeviceId,
    ) -> Result<Option<SecretBytes>, SecurityError> {
        let path = self.path_for(device_id);
        if !path.exists() {
            return Ok(None);
        }

        let protected = std::fs::read(path).map_err(secret_error)?;
        let secret = dpapi::unprotect_secret(&protected)?;
        Ok(Some(SecretBytes::from_bytes(secret)))
    }
}

#[cfg(not(windows))]
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

fn stable_device_id_hash(device_id: &DeviceId) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(device_id.as_str().as_bytes());
    hasher.finalize()
}

fn secret_error(error: impl std::fmt::Display) -> SecurityError {
    SecurityError::SecretStore(error.to_string())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(windows)]
fn default_secret_dir() -> Result<PathBuf, SecurityError> {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .ok_or_else(|| SecurityError::SecretStore("LOCALAPPDATA/APPDATA is not set".to_string()))?;

    Ok(base.join("GlyphRay").join("secrets"))
}

#[cfg(windows)]
mod dpapi {
    use super::{secret_error, SecurityError};
    use std::slice;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    pub fn protect_secret(plaintext: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let input = blob_from_slice(plaintext);
        let mut output = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(secret_error)?;

            copy_blob_and_free(output)
        }
    }

    pub fn unprotect_secret(protected: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let input = blob_from_slice(protected);
        let mut output = CRYPT_INTEGER_BLOB::default();

        unsafe {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
            .map_err(secret_error)?;

            copy_blob_and_free(output)
        }
    }

    fn blob_from_slice(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        }
    }

    unsafe fn copy_blob_and_free(blob: CRYPT_INTEGER_BLOB) -> Result<Vec<u8>, SecurityError> {
        if blob.pbData.is_null() {
            return Err(SecurityError::SecretStore(
                "DPAPI returned a null output buffer".to_string(),
            ));
        }

        let bytes = slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(blob.pbData.cast()));
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_secret_store_roundtrips_device_secret() {
        let root = unique_temp_dir();
        let _ = std::fs::remove_dir_all(&root);

        let device_id = DeviceId::new("test-device");
        let secret = SecretBytes::from_bytes(b"glyphray test secret".to_vec());
        let mut store = PlatformSecretStore::open_at(&root).expect("open store");

        store
            .put_device_secret(&device_id, secret.clone())
            .expect("put secret");
        let loaded = store
            .get_device_secret(&device_id)
            .expect("get secret")
            .expect("secret exists");
        assert_eq!(loaded.expose(), secret.expose());

        #[cfg(windows)]
        {
            let reopened = PlatformSecretStore::open_at(&root).expect("reopen store");
            let loaded = reopened
                .get_device_secret(&device_id)
                .expect("get secret")
                .expect("secret exists");
            assert_eq!(loaded.expose(), secret.expose());
            assert!(secret_file_count(&root) > 0);
        }

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn host_identity_is_persisted_by_the_platform_store() {
        let root = unique_temp_dir().join("host-identity");
        let _ = std::fs::remove_dir_all(&root);
        let first_fingerprint = {
            let mut store = PlatformSecretStore::open_at(&root).expect("open");
            load_or_create_host_identity(&mut store)
                .expect("identity")
                .fingerprint()
                .expect("fingerprint")
        };

        #[cfg(windows)]
        {
            let mut store = PlatformSecretStore::open_at(&root).expect("reopen");
            let second = load_or_create_host_identity(&mut store)
                .expect("identity")
                .fingerprint()
                .expect("fingerprint");
            assert_eq!(first_fingerprint, second);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn corrupt_host_identity_is_quarantined_and_regenerated() {
        let root = unique_temp_dir().join("corrupt-host-identity");
        let _ = std::fs::remove_dir_all(&root);
        let mut store = PlatformSecretStore::open_at(&root).expect("open");
        let device_id = DeviceId::new("glyphray-host-identity-v1");
        std::fs::write(store.path_for(&device_id), b"not a DPAPI blob")
            .expect("write corrupt identity");

        let recovered = load_or_recover_host_identity(&mut store).expect("recover identity");
        let backup = recovered.quarantined_path.expect("quarantine path");
        assert_eq!(
            std::fs::read(backup).expect("backup bytes"),
            b"not a DPAPI blob"
        );
        let reopened = load_or_recover_host_identity(&mut store).expect("reopen identity");
        assert!(reopened.quarantined_path.is_none());
        assert_eq!(
            recovered.identity.fingerprint().expect("fingerprint"),
            reopened.identity.fingerprint().expect("fingerprint")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn unique_temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "glyphray-platform-secret-store-{}-{}",
            std::process::id(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[cfg(windows)]
    fn secret_file_count(root: &std::path::Path) -> usize {
        std::fs::read_dir(root)
            .map(|entries| entries.filter_map(Result::ok).count())
            .unwrap_or(0)
    }
}
