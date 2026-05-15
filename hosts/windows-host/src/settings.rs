use glyphray_protocol::{ColorSpace, EncoderConfig, VideoCodec};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostSettings {
    pub encoder_override: Option<EncoderConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum HostSettingsError {
    #[error("host settings I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("host settings parse failed: {0}")]
    Parse(String),
}

pub struct HostSettingsStore {
    path: PathBuf,
}

impl HostSettingsStore {
    pub fn open() -> Result<Self, HostSettingsError> {
        Self::open_at(default_settings_path())
    }

    pub fn open_at(path: impl Into<PathBuf>) -> Result<Self, HostSettingsError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    pub fn load(&self) -> Result<HostSettings, HostSettingsError> {
        if !self.path.exists() {
            return Ok(HostSettings::default());
        }
        parse_settings(&std::fs::read_to_string(&self.path)?)
    }

    pub fn save(&self, settings: &HostSettings) -> Result<(), HostSettingsError> {
        std::fs::write(&self.path, serialize_settings(settings))?;
        Ok(())
    }

    pub fn save_encoder_override(
        &self,
        config: EncoderConfig,
    ) -> Result<HostSettings, HostSettingsError> {
        let mut settings = self.load()?;
        settings.encoder_override = Some(config);
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn clear_encoder_override(&self) -> Result<HostSettings, HostSettingsError> {
        let mut settings = self.load()?;
        settings.encoder_override = None;
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn serialize_settings(settings: &HostSettings) -> String {
    let mut output = String::from("# GlyphRay host settings v1\n");
    if let Some(config) = settings.encoder_override.as_ref() {
        output.push_str(&format!("encoder.display_id={}\n", config.display_id));
        output.push_str(&format!(
            "encoder.codec={}\n",
            video_codec_name(config.codec)
        ));
        output.push_str(&format!(
            "encoder.color_space={}\n",
            color_space_name(config.color_space)
        ));
        output.push_str(&format!("encoder.width={}\n", config.width));
        output.push_str(&format!("encoder.height={}\n", config.height));
        output.push_str(&format!("encoder.max_fps={}\n", config.max_fps));
        output.push_str(&format!(
            "encoder.target_bitrate_kbps={}\n",
            config.target_bitrate_kbps
        ));
        output.push_str(&format!(
            "encoder.keyframe_interval_ms={}\n",
            config.keyframe_interval_ms
        ));
        output.push_str(&format!("encoder.low_latency={}\n", config.low_latency));
    }
    output
}

fn parse_settings(raw: &str) -> Result<HostSettings, HostSettingsError> {
    let mut values = HashMap::new();
    for (line_index, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(HostSettingsError::Parse(format!(
                "line {} is not key=value",
                line_index + 1
            )));
        };
        values.insert(key.trim().to_string(), value.trim().to_string());
    }

    let has_encoder_override = values.keys().any(|key| key.starts_with("encoder."));
    Ok(HostSettings {
        encoder_override: if has_encoder_override {
            Some(parse_encoder_config(&values)?)
        } else {
            None
        },
    })
}

fn parse_encoder_config(
    values: &HashMap<String, String>,
) -> Result<EncoderConfig, HostSettingsError> {
    Ok(EncoderConfig {
        display_id: parse_required(values, "encoder.display_id")?,
        codec: parse_video_codec(required(values, "encoder.codec")?)?,
        color_space: parse_color_space(required(values, "encoder.color_space")?)?,
        width: parse_required(values, "encoder.width")?,
        height: parse_required(values, "encoder.height")?,
        max_fps: parse_required::<u16>(values, "encoder.max_fps")?.clamp(30, 120),
        target_bitrate_kbps: parse_required::<u32>(values, "encoder.target_bitrate_kbps")?
            .clamp(4_000, 120_000),
        keyframe_interval_ms: parse_required::<u32>(values, "encoder.keyframe_interval_ms")?
            .clamp(250, 10_000),
        low_latency: parse_required(values, "encoder.low_latency")?,
    })
}

fn required<'a>(
    values: &'a HashMap<String, String>,
    key: &str,
) -> Result<&'a str, HostSettingsError> {
    values
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| HostSettingsError::Parse(format!("missing {key}")))
}

fn parse_required<T>(values: &HashMap<String, String>, key: &str) -> Result<T, HostSettingsError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    required(values, key)?
        .parse::<T>()
        .map_err(|error| HostSettingsError::Parse(format!("{key}: {error}")))
}

fn video_codec_name(codec: VideoCodec) -> &'static str {
    match codec {
        VideoCodec::H264 => "H264",
        VideoCodec::H265 => "H265",
        VideoCodec::Av1 => "Av1",
    }
}

fn parse_video_codec(value: &str) -> Result<VideoCodec, HostSettingsError> {
    match value {
        "H264" => Ok(VideoCodec::H264),
        "H265" => Ok(VideoCodec::H265),
        "Av1" | "AV1" => Ok(VideoCodec::Av1),
        _ => Err(HostSettingsError::Parse(format!(
            "unknown encoder.codec {value}"
        ))),
    }
}

fn color_space_name(color_space: ColorSpace) -> &'static str {
    match color_space {
        ColorSpace::Srgb => "Srgb",
        ColorSpace::DisplayP3 => "DisplayP3",
        ColorSpace::Rec709 => "Rec709",
        ColorSpace::Rec2020Pq => "Rec2020Pq",
    }
}

fn parse_color_space(value: &str) -> Result<ColorSpace, HostSettingsError> {
    match value {
        "Srgb" | "sRGB" => Ok(ColorSpace::Srgb),
        "DisplayP3" => Ok(ColorSpace::DisplayP3),
        "Rec709" => Ok(ColorSpace::Rec709),
        "Rec2020Pq" => Ok(ColorSpace::Rec2020Pq),
        _ => Err(HostSettingsError::Parse(format!(
            "unknown encoder.color_space {value}"
        ))),
    }
}

fn default_settings_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("GlyphRay").join("host-settings.conf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_settings_round_trip_encoder_override() {
        let config = EncoderConfig {
            display_id: 2,
            codec: VideoCodec::H264,
            color_space: ColorSpace::DisplayP3,
            width: 2560,
            height: 1440,
            max_fps: 120,
            target_bitrate_kbps: 35_000,
            keyframe_interval_ms: 500,
            low_latency: true,
        };
        let settings = HostSettings {
            encoder_override: Some(config),
        };

        let serialized = serialize_settings(&settings);
        let parsed = parse_settings(&serialized).expect("parse settings");

        assert_eq!(parsed, settings);
    }

    #[test]
    fn host_settings_store_persists_and_clears_encoder_override() {
        let path = std::env::temp_dir().join(format!(
            "glyphray-host-settings-test-{}.conf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let store = HostSettingsStore::open_at(&path).expect("open settings store");
        let config = EncoderConfig {
            display_id: 1,
            codec: VideoCodec::H264,
            color_space: ColorSpace::Rec709,
            width: 1920,
            height: 1080,
            max_fps: 60,
            target_bitrate_kbps: 18_000,
            keyframe_interval_ms: 1_000,
            low_latency: true,
        };

        store
            .save_encoder_override(config.clone())
            .expect("save encoder override");
        assert_eq!(store.load().expect("load").encoder_override, Some(config));

        store
            .clear_encoder_override()
            .expect("clear encoder override");
        assert_eq!(store.load().expect("load").encoder_override, None);
        let _ = std::fs::remove_file(path);
    }
}
