use std::path::Path;

const STARTUP_VALUE_NAME: &str = "GlyphRay Host";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupRegistration {
    pub enabled: bool,
    pub command: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("startup registration is only supported on Windows")]
    UnsupportedPlatform,
    #[error("failed to locate the current executable: {0}")]
    CurrentExe(std::io::Error),
    #[error("Windows registry operation failed with code {0}")]
    Registry(u32),
    #[error("startup registry value was not a string")]
    UnexpectedRegistryType,
}

pub struct StartupManager;

impl StartupManager {
    pub fn status() -> Result<StartupRegistration, StartupError> {
        platform::status()
    }

    pub fn enable() -> Result<StartupRegistration, StartupError> {
        let exe = std::env::current_exe().map_err(StartupError::CurrentExe)?;
        platform::enable(&startup_command_for_exe(&exe))?;
        Self::status()
    }

    pub fn disable() -> Result<StartupRegistration, StartupError> {
        platform::disable()?;
        Self::status()
    }
}

pub fn startup_command_for_exe(exe: &Path) -> String {
    format!("\"{}\" serve", exe.display())
}

#[cfg(not(windows))]
mod platform {
    use super::{StartupError, StartupRegistration};

    pub fn status() -> Result<StartupRegistration, StartupError> {
        Err(StartupError::UnsupportedPlatform)
    }

    pub fn enable(_command: &str) -> Result<(), StartupError> {
        Err(StartupError::UnsupportedPlatform)
    }

    pub fn disable() -> Result<(), StartupError> {
        Err(StartupError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{StartupError, StartupRegistration, STARTUP_VALUE_NAME};
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
    };

    const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

    pub fn status() -> Result<StartupRegistration, StartupError> {
        let key = open_run_key(KEY_READ)?;
        let command = query_string_value(&key, STARTUP_VALUE_NAME)?;
        Ok(StartupRegistration {
            enabled: command.is_some(),
            command,
        })
    }

    pub fn enable(command: &str) -> Result<(), StartupError> {
        let key = open_run_key(KEY_SET_VALUE)?;
        let name = wide_null(STARTUP_VALUE_NAME);
        let value = reg_sz_bytes(command);
        let status =
            unsafe { RegSetValueExW(key.raw, PCWSTR(name.as_ptr()), 0, REG_SZ, Some(&value)) };
        check(status)
    }

    pub fn disable() -> Result<(), StartupError> {
        let key = open_run_key(KEY_SET_VALUE)?;
        let name = wide_null(STARTUP_VALUE_NAME);
        let status = unsafe { RegDeleteValueW(key.raw, PCWSTR(name.as_ptr())) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(status)
    }

    struct RegistryKey {
        raw: HKEY,
    }

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            let _ = unsafe { RegCloseKey(self.raw) };
        }
    }

    fn open_run_key(
        access: windows::Win32::System::Registry::REG_SAM_FLAGS,
    ) -> Result<RegistryKey, StartupError> {
        let subkey = wide_null(RUN_SUBKEY);
        let mut raw = HKEY::default();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                0,
                access,
                &mut raw,
            )
        };
        check(status)?;
        Ok(RegistryKey { raw })
    }

    fn query_string_value(
        key: &RegistryKey,
        value_name: &str,
    ) -> Result<Option<String>, StartupError> {
        let name = wide_null(value_name);
        let mut value_type = REG_SZ;
        let mut byte_len = 0_u32;
        let status = unsafe {
            RegQueryValueExW(
                key.raw,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut value_type),
                None,
                Some(&mut byte_len),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status)?;
        if value_type != REG_SZ {
            return Err(StartupError::UnexpectedRegistryType);
        }

        let mut bytes = vec![0_u8; byte_len as usize];
        let status = unsafe {
            RegQueryValueExW(
                key.raw,
                PCWSTR(name.as_ptr()),
                None,
                Some(&mut value_type),
                Some(bytes.as_mut_ptr()),
                Some(&mut byte_len),
            )
        };
        check(status)?;
        Ok(Some(string_from_reg_sz_bytes(&bytes)))
    }

    fn check(status: windows::Win32::Foundation::WIN32_ERROR) -> Result<(), StartupError> {
        if status == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(StartupError::Registry(status.0))
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn reg_sz_bytes(value: &str) -> Vec<u8> {
        wide_null(value)
            .into_iter()
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    fn string_from_reg_sz_bytes(bytes: &[u8]) -> String {
        let words: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|word| *word != 0)
            .collect();
        String::from_utf16_lossy(&words)
    }

    #[cfg(test)]
    mod tests {
        use super::{reg_sz_bytes, string_from_reg_sz_bytes};

        #[test]
        fn reg_sz_encoding_round_trips() {
            let command = "\"C:\\Program Files\\GlyphRay\\glyphray-windows-host.exe\" serve";
            let encoded = reg_sz_bytes(command);
            assert_eq!(string_from_reg_sz_bytes(&encoded), command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::startup_command_for_exe;
    use std::path::Path;

    #[test]
    fn startup_command_quotes_executable_and_appends_serve() {
        let command = startup_command_for_exe(Path::new("C:\\Program Files\\GlyphRay\\host.exe"));
        assert_eq!(command, "\"C:\\Program Files\\GlyphRay\\host.exe\" serve");
    }
}
