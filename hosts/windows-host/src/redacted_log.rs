use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveredState {
    HostIdentity,
    HostSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactedLogEvent {
    HostStarted,
    StateRecovered(RecoveredState),
    UnexpectedTermination,
}

impl RedactedLogEvent {
    fn label(self) -> &'static str {
        match self {
            Self::HostStarted => "host_started",
            Self::StateRecovered(RecoveredState::HostIdentity) => "host_identity_recovered",
            Self::StateRecovered(RecoveredState::HostSettings) => "host_settings_recovered",
            Self::UnexpectedTermination => "unexpected_termination",
        }
    }
}

pub struct RedactedEventLog {
    path: PathBuf,
    file: File,
}

impl RedactedEventLog {
    pub fn open_default() -> io::Result<Self> {
        Self::open_at(default_log_path())
    }

    pub fn open_at(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        rotate_if_needed(&path)?;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, file })
    }

    pub fn append(&mut self, event: RedactedLogEvent) -> io::Result<()> {
        append_event(&mut self.file, event)
    }

    pub fn install_panic_hook(&self) {
        let path = self.path.clone();
        std::panic::set_hook(Box::new(move |_| {
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
                let _ = append_event(&mut file, RedactedLogEvent::UnexpectedTermination);
            }
            eprintln!(
                "GlyphRay terminated unexpectedly. A redacted local crash marker was written."
            );
        }));
    }
}

fn append_event(file: &mut File, event: RedactedLogEvent) -> io::Result<()> {
    writeln!(
        file,
        "timestamp_ms={} pid={} event={}",
        unix_now_ms(),
        std::process::id(),
        event.label()
    )?;
    file.sync_data()
}

fn rotate_if_needed(path: &Path) -> io::Result<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_LOG_BYTES {
        return Ok(());
    }
    let previous = path.with_extension("previous.log");
    let _ = std::fs::remove_file(&previous);
    std::fs::rename(path, previous)
}

fn default_log_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("GlyphRay").join("logs").join("host-events.log")
}

fn unix_now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_log_contains_only_fixed_schema_fields() {
        let path =
            std::env::temp_dir().join(format!("glyphray-redacted-log-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut log = RedactedEventLog::open_at(&path).expect("open log");
        log.append(RedactedLogEvent::HostStarted).expect("append");
        log.append(RedactedLogEvent::StateRecovered(
            RecoveredState::HostSettings,
        ))
        .expect("append recovery");
        drop(log);

        let text = std::fs::read_to_string(&path).expect("read log");
        assert!(text.contains("event=host_started"));
        assert!(text.contains("event=host_settings_recovered"));
        assert!(!text.contains("keyboard"));
        assert!(!text.contains("secret"));
        let _ = std::fs::remove_file(path);
    }
}
