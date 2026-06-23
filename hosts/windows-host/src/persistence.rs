use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let temporary = temporary_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

pub fn quarantine_file(path: &Path, category: &str) -> io::Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let category = category
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .collect::<String>();
    let category = if category.is_empty() {
        "corrupt"
    } else {
        category.as_str()
    };
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = unix_now_ms();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("glyphray-state");
    let backup = path.with_file_name(format!(
        "{file_name}.{category}-{timestamp}-{}-{sequence}",
        std::process::id()
    ));
    std::fs::rename(path, &backup)?;
    Ok(Some(backup))
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("glyphray-state");
    path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ))
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        MoveFileExW(
            PCWSTR(source.as_ptr()),
            PCWSTR(destination.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(io::Error::other)
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    std::fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_existing_content_without_temp_files() {
        let root = std::env::temp_dir().join(format!(
            "glyphray-atomic-write-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("state.bin");
        atomic_write(&path, b"first").expect("first write");
        atomic_write(&path, b"second").expect("replacement write");
        assert_eq!(std::fs::read(&path).expect("read"), b"second");
        let entries = std::fs::read_dir(&root)
            .expect("read root")
            .filter_map(Result::ok)
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn quarantine_preserves_corrupt_bytes() {
        let root = std::env::temp_dir().join(format!(
            "glyphray-quarantine-{}-{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let path = root.join("settings.conf");
        atomic_write(&path, b"broken").expect("write");
        let backup = quarantine_file(&path, "corrupt")
            .expect("quarantine")
            .expect("backup path");
        assert!(!path.exists());
        assert_eq!(std::fs::read(backup).expect("backup bytes"), b"broken");
        let _ = std::fs::remove_dir_all(root);
    }
}
