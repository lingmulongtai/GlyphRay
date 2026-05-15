use glyphray_protocol::{ColorSpace, EncoderConfig, VideoCodec};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HostSettings {
    pub encoder_override: Option<EncoderConfig>,
    pub encoder_presets: Vec<EncoderPreset>,
    pub trusted_devices: Vec<TrustedDevice>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncoderPreset {
    pub name: String,
    pub config: EncoderConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedDevice {
    pub id: String,
    pub label: String,
    pub last_peer: String,
    pub public_key_fingerprint: Option<String>,
    pub approved_unix_ms: u64,
    pub permissions: TrustedDevicePermissions,
}

impl TrustedDevice {
    pub fn approved_now(
        id: impl Into<String>,
        label: impl Into<String>,
        last_peer: impl Into<String>,
        public_key_fingerprint: Option<String>,
    ) -> Result<Self, HostSettingsError> {
        Ok(Self {
            id: normalize_trusted_device_id(&id.into())?,
            label: normalize_trusted_device_label(&label.into()),
            last_peer: normalize_trusted_device_label(&last_peer.into()),
            public_key_fingerprint: public_key_fingerprint
                .map(|fingerprint| normalize_public_key_fingerprint(&fingerprint))
                .transpose()?,
            approved_unix_ms: unix_now_ms(),
            permissions: TrustedDevicePermissions::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedDevicePermissions {
    pub allow_pen: bool,
    pub allow_touch: bool,
    pub allow_keyboard: bool,
    pub allow_mouse: bool,
    pub allow_gamepad: bool,
}

impl Default for TrustedDevicePermissions {
    fn default() -> Self {
        Self {
            allow_pen: true,
            allow_touch: true,
            allow_keyboard: true,
            allow_mouse: true,
            allow_gamepad: true,
        }
    }
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

    pub fn save_encoder_preset(
        &self,
        name: &str,
        config: EncoderConfig,
    ) -> Result<HostSettings, HostSettingsError> {
        let mut settings = self.load()?;
        upsert_encoder_preset(
            &mut settings.encoder_presets,
            EncoderPreset {
                name: normalize_preset_name(name)?,
                config,
            },
        );
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn delete_encoder_preset(
        &self,
        name: &str,
    ) -> Result<(HostSettings, bool), HostSettingsError> {
        let mut settings = self.load()?;
        let preset_name = normalize_preset_name(name)?;
        let original_len = settings.encoder_presets.len();
        settings
            .encoder_presets
            .retain(|preset| !preset_name_eq(&preset.name, &preset_name));
        let removed = settings.encoder_presets.len() != original_len;
        self.save(&settings)?;
        Ok((settings, removed))
    }

    pub fn load_encoder_preset(
        &self,
        name: &str,
    ) -> Result<Option<EncoderConfig>, HostSettingsError> {
        let settings = self.load()?;
        let preset_name = normalize_preset_name(name)?;
        Ok(settings
            .encoder_presets
            .iter()
            .find(|preset| preset_name_eq(&preset.name, &preset_name))
            .map(|preset| preset.config.clone()))
    }

    pub fn upsert_trusted_device(
        &self,
        device: TrustedDevice,
    ) -> Result<HostSettings, HostSettingsError> {
        let mut settings = self.load()?;
        upsert_trusted_device(&mut settings.trusted_devices, device);
        self.save(&settings)?;
        Ok(settings)
    }

    pub fn forget_trusted_device(
        &self,
        id: &str,
    ) -> Result<(HostSettings, bool), HostSettingsError> {
        let mut settings = self.load()?;
        let id = normalize_trusted_device_id(id)?;
        let original_len = settings.trusted_devices.len();
        settings
            .trusted_devices
            .retain(|device| !trusted_device_id_eq(&device.id, &id));
        let removed = settings.trusted_devices.len() != original_len;
        self.save(&settings)?;
        Ok((settings, removed))
    }

    pub fn clear_trusted_devices(&self) -> Result<HostSettings, HostSettingsError> {
        let mut settings = self.load()?;
        settings.trusted_devices.clear();
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
        write_encoder_config(&mut output, "encoder.", config);
    }
    for (index, preset) in settings.encoder_presets.iter().enumerate() {
        output.push_str(&format!("encoder_preset.{index}.name={}\n", preset.name));
        write_encoder_config(
            &mut output,
            &format!("encoder_preset.{index}."),
            &preset.config,
        );
    }
    for (index, device) in settings.trusted_devices.iter().enumerate() {
        output.push_str(&format!("trusted_device.{index}.id={}\n", device.id));
        output.push_str(&format!("trusted_device.{index}.label={}\n", device.label));
        output.push_str(&format!(
            "trusted_device.{index}.last_peer={}\n",
            device.last_peer
        ));
        if let Some(fingerprint) = device.public_key_fingerprint.as_ref() {
            output.push_str(&format!(
                "trusted_device.{index}.public_key_fingerprint={}\n",
                fingerprint
            ));
        }
        output.push_str(&format!(
            "trusted_device.{index}.approved_unix_ms={}\n",
            device.approved_unix_ms
        ));
        output.push_str(&format!(
            "trusted_device.{index}.allow_pen={}\n",
            device.permissions.allow_pen
        ));
        output.push_str(&format!(
            "trusted_device.{index}.allow_touch={}\n",
            device.permissions.allow_touch
        ));
        output.push_str(&format!(
            "trusted_device.{index}.allow_keyboard={}\n",
            device.permissions.allow_keyboard
        ));
        output.push_str(&format!(
            "trusted_device.{index}.allow_mouse={}\n",
            device.permissions.allow_mouse
        ));
        output.push_str(&format!(
            "trusted_device.{index}.allow_gamepad={}\n",
            device.permissions.allow_gamepad
        ));
    }
    output
}

fn write_encoder_config(output: &mut String, prefix: &str, config: &EncoderConfig) {
    output.push_str(&format!("{prefix}display_id={}\n", config.display_id));
    output.push_str(&format!(
        "{prefix}codec={}\n",
        video_codec_name(config.codec)
    ));
    output.push_str(&format!(
        "{prefix}color_space={}\n",
        color_space_name(config.color_space)
    ));
    output.push_str(&format!("{prefix}width={}\n", config.width));
    output.push_str(&format!("{prefix}height={}\n", config.height));
    output.push_str(&format!("{prefix}max_fps={}\n", config.max_fps));
    output.push_str(&format!(
        "{prefix}target_bitrate_kbps={}\n",
        config.target_bitrate_kbps
    ));
    output.push_str(&format!(
        "{prefix}keyframe_interval_ms={}\n",
        config.keyframe_interval_ms
    ));
    output.push_str(&format!("{prefix}low_latency={}\n", config.low_latency));
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
            Some(parse_encoder_config_with_prefix(&values, "encoder.")?)
        } else {
            None
        },
        encoder_presets: parse_encoder_presets(&values)?,
        trusted_devices: parse_trusted_devices(&values)?,
    })
}

fn parse_encoder_config_with_prefix(
    values: &HashMap<String, String>,
    prefix: &str,
) -> Result<EncoderConfig, HostSettingsError> {
    Ok(EncoderConfig {
        display_id: parse_required(values, &format!("{prefix}display_id"))?,
        codec: parse_video_codec(required(values, &format!("{prefix}codec"))?)?,
        color_space: parse_color_space(required(values, &format!("{prefix}color_space"))?)?,
        width: parse_required(values, &format!("{prefix}width"))?,
        height: parse_required(values, &format!("{prefix}height"))?,
        max_fps: parse_required::<u16>(values, &format!("{prefix}max_fps"))?.clamp(30, 120),
        target_bitrate_kbps: parse_required::<u32>(
            values,
            &format!("{prefix}target_bitrate_kbps"),
        )?
        .clamp(4_000, 120_000),
        keyframe_interval_ms: parse_required::<u32>(
            values,
            &format!("{prefix}keyframe_interval_ms"),
        )?
        .clamp(250, 10_000),
        low_latency: parse_required(values, &format!("{prefix}low_latency"))?,
    })
}

fn parse_encoder_presets(
    values: &HashMap<String, String>,
) -> Result<Vec<EncoderPreset>, HostSettingsError> {
    let mut groups: BTreeMap<u32, HashMap<String, String>> = BTreeMap::new();
    for (key, value) in values {
        let Some(rest) = key.strip_prefix("encoder_preset.") else {
            continue;
        };
        let Some((index, field)) = rest.split_once('.') else {
            return Err(HostSettingsError::Parse(format!(
                "invalid encoder preset key {key}"
            )));
        };
        let index = index
            .parse::<u32>()
            .map_err(|error| HostSettingsError::Parse(format!("{key}: {error}")))?;
        groups
            .entry(index)
            .or_default()
            .insert(field.to_string(), value.clone());
    }

    let mut presets = Vec::new();
    for (index, fields) in groups {
        let name = normalize_preset_name(required(&fields, "name")?)?;
        let config = parse_encoder_config_fields(&fields).map_err(|error| {
            HostSettingsError::Parse(format!("encoder_preset.{index}: {error}"))
        })?;
        upsert_encoder_preset(&mut presets, EncoderPreset { name, config });
    }
    Ok(presets)
}

fn parse_encoder_config_fields(
    values: &HashMap<String, String>,
) -> Result<EncoderConfig, HostSettingsError> {
    Ok(EncoderConfig {
        display_id: parse_required(values, "display_id")?,
        codec: parse_video_codec(required(values, "codec")?)?,
        color_space: parse_color_space(required(values, "color_space")?)?,
        width: parse_required(values, "width")?,
        height: parse_required(values, "height")?,
        max_fps: parse_required::<u16>(values, "max_fps")?.clamp(30, 120),
        target_bitrate_kbps: parse_required::<u32>(values, "target_bitrate_kbps")?
            .clamp(4_000, 120_000),
        keyframe_interval_ms: parse_required::<u32>(values, "keyframe_interval_ms")?
            .clamp(250, 10_000),
        low_latency: parse_required(values, "low_latency")?,
    })
}

fn upsert_encoder_preset(presets: &mut Vec<EncoderPreset>, preset: EncoderPreset) {
    if let Some(existing) = presets
        .iter_mut()
        .find(|existing| preset_name_eq(&existing.name, &preset.name))
    {
        *existing = preset;
    } else {
        presets.push(preset);
    }
    presets.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
}

fn parse_trusted_devices(
    values: &HashMap<String, String>,
) -> Result<Vec<TrustedDevice>, HostSettingsError> {
    let mut groups: BTreeMap<u32, HashMap<String, String>> = BTreeMap::new();
    for (key, value) in values {
        let Some(rest) = key.strip_prefix("trusted_device.") else {
            continue;
        };
        let Some((index, field)) = rest.split_once('.') else {
            return Err(HostSettingsError::Parse(format!(
                "invalid trusted device key {key}"
            )));
        };
        let index = index
            .parse::<u32>()
            .map_err(|error| HostSettingsError::Parse(format!("{key}: {error}")))?;
        groups
            .entry(index)
            .or_default()
            .insert(field.to_string(), value.clone());
    }

    let mut devices = Vec::new();
    for (index, fields) in groups {
        let device = parse_trusted_device_fields(&fields).map_err(|error| {
            HostSettingsError::Parse(format!("trusted_device.{index}: {error}"))
        })?;
        upsert_trusted_device(&mut devices, device);
    }
    Ok(devices)
}

fn parse_trusted_device_fields(
    values: &HashMap<String, String>,
) -> Result<TrustedDevice, HostSettingsError> {
    Ok(TrustedDevice {
        id: normalize_trusted_device_id(required(values, "id")?)?,
        label: normalize_trusted_device_label(required(values, "label")?),
        last_peer: normalize_trusted_device_label(required(values, "last_peer")?),
        public_key_fingerprint: values
            .get("public_key_fingerprint")
            .map(|fingerprint| normalize_public_key_fingerprint(fingerprint))
            .transpose()?,
        approved_unix_ms: parse_required(values, "approved_unix_ms")?,
        permissions: TrustedDevicePermissions {
            allow_pen: parse_optional_bool(values, "allow_pen", true)?,
            allow_touch: parse_optional_bool(values, "allow_touch", true)?,
            allow_keyboard: parse_optional_bool(values, "allow_keyboard", true)?,
            allow_mouse: parse_optional_bool(values, "allow_mouse", true)?,
            allow_gamepad: parse_optional_bool(values, "allow_gamepad", true)?,
        },
    })
}

fn parse_optional_bool(
    values: &HashMap<String, String>,
    key: &str,
    default: bool,
) -> Result<bool, HostSettingsError> {
    match values.get(key) {
        Some(value) => value
            .parse::<bool>()
            .map_err(|error| HostSettingsError::Parse(format!("{key}: {error}"))),
        None => Ok(default),
    }
}

fn upsert_trusted_device(devices: &mut Vec<TrustedDevice>, device: TrustedDevice) {
    if let Some(existing) = devices
        .iter_mut()
        .find(|existing| trusted_device_id_eq(&existing.id, &device.id))
    {
        *existing = device;
    } else {
        devices.push(device);
    }
    devices.sort_by(|left, right| left.label.to_lowercase().cmp(&right.label.to_lowercase()));
}

fn normalize_preset_name(name: &str) -> Result<String, HostSettingsError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(HostSettingsError::Parse(
            "encoder preset name cannot be empty".to_string(),
        ));
    }
    if name.len() > 64 {
        return Err(HostSettingsError::Parse(
            "encoder preset name must be 64 characters or shorter".to_string(),
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(HostSettingsError::Parse(
            "encoder preset name must use ASCII letters, numbers, '-', '_', or '.'".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn preset_name_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn normalize_trusted_device_id(id: &str) -> Result<String, HostSettingsError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(HostSettingsError::Parse(
            "trusted device id cannot be empty".to_string(),
        ));
    }
    if id.len() > 128 {
        return Err(HostSettingsError::Parse(
            "trusted device id must be 128 characters or shorter".to_string(),
        ));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(HostSettingsError::Parse(
            "trusted device id must use ASCII letters, numbers, '-', '_', or '.'".to_string(),
        ));
    }
    Ok(id.to_string())
}

fn normalize_trusted_device_label(label: &str) -> String {
    let mut normalized = label
        .chars()
        .map(|ch| {
            if matches!(ch, '\r' | '\n' | '\t') {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>();
    normalized.truncate(80);
    let normalized = normalized.trim();
    if normalized.is_empty() {
        "Unknown device".to_string()
    } else {
        normalized.to_string()
    }
}

fn trusted_device_id_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn normalize_public_key_fingerprint(fingerprint: &str) -> Result<String, HostSettingsError> {
    let fingerprint = fingerprint.trim().to_ascii_lowercase();
    if fingerprint.len() != 64
        || !fingerprint
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        return Err(HostSettingsError::Parse(
            "public key fingerprint must be a 64-character lowercase hex SHA-256 digest"
                .to_string(),
        ));
    }
    Ok(fingerprint)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
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
            encoder_presets: vec![EncoderPreset {
                name: "studio-120".to_string(),
                config: EncoderConfig {
                    display_id: 1,
                    codec: VideoCodec::H264,
                    color_space: ColorSpace::Rec709,
                    width: 1920,
                    height: 1080,
                    max_fps: 120,
                    target_bitrate_kbps: 45_000,
                    keyframe_interval_ms: 500,
                    low_latency: true,
                },
            }],
            trusted_devices: vec![TrustedDevice {
                id: "trusted-tablet".to_string(),
                label: "Studio Tablet".to_string(),
                last_peer: "192.168.1.20:44999".to_string(),
                public_key_fingerprint: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                approved_unix_ms: 1_770_000_000_000,
                permissions: TrustedDevicePermissions::default(),
            }],
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

    #[test]
    fn host_settings_store_upserts_and_deletes_named_presets() {
        let path = std::env::temp_dir().join(format!(
            "glyphray-host-settings-preset-test-{}.conf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let store = HostSettingsStore::open_at(&path).expect("open settings store");
        let first = EncoderConfig {
            display_id: 0,
            codec: VideoCodec::H264,
            color_space: ColorSpace::Rec709,
            width: 1920,
            height: 1080,
            max_fps: 60,
            target_bitrate_kbps: 18_000,
            keyframe_interval_ms: 1_000,
            low_latency: true,
        };
        let second = EncoderConfig {
            max_fps: 120,
            target_bitrate_kbps: 35_000,
            ..first.clone()
        };

        store
            .save_encoder_preset("Studio-High", first)
            .expect("save preset");
        store
            .save_encoder_preset("studio-high", second.clone())
            .expect("update preset");

        let settings = store.load().expect("load");
        assert_eq!(settings.encoder_presets.len(), 1);
        assert_eq!(
            store
                .load_encoder_preset("STUDIO-HIGH")
                .expect("load preset"),
            Some(second)
        );

        let (_, removed) = store
            .delete_encoder_preset("studio-high")
            .expect("delete preset");
        assert!(removed);
        assert!(store
            .load()
            .expect("load after delete")
            .encoder_presets
            .is_empty());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn host_settings_store_upserts_and_forgets_trusted_devices() {
        let path = std::env::temp_dir().join(format!(
            "glyphray-host-settings-trust-test-{}.conf",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let store = HostSettingsStore::open_at(&path).expect("open settings store");
        let first = TrustedDevice {
            id: "trusted-tablet".to_string(),
            label: "Tablet".to_string(),
            last_peer: "192.168.1.20:44999".to_string(),
            public_key_fingerprint: Some(
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            ),
            approved_unix_ms: 10,
            permissions: TrustedDevicePermissions::default(),
        };
        let second = TrustedDevice {
            label: "Tablet Renamed".to_string(),
            approved_unix_ms: 20,
            ..first.clone()
        };

        store
            .upsert_trusted_device(first)
            .expect("save trusted device");
        store
            .upsert_trusted_device(second.clone())
            .expect("update trusted device");

        let settings = store.load().expect("load");
        assert_eq!(settings.trusted_devices, vec![second]);

        let (_, removed) = store
            .forget_trusted_device("TRUSTED-TABLET")
            .expect("forget trusted device");
        assert!(removed);
        assert!(store
            .load()
            .expect("load after forget")
            .trusted_devices
            .is_empty());
        let _ = std::fs::remove_file(path);
    }
}
