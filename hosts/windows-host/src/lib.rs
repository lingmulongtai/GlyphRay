pub mod audio;
pub mod backend;
pub mod capture;
pub mod config;
pub mod diagnostics;
pub mod encoder;
pub mod input;
pub mod pairing;
pub mod permission_ui;
pub mod persistence;
pub mod redacted_log;
pub mod secrets;
pub mod settings;
pub mod startup;
pub mod streaming;

pub use config::{EncoderPreference, HostConfig};
