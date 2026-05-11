#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostConfig {
    pub host_name: String,
    pub default_display_id: u32,
    pub encoder_preference: EncoderPreference,
    pub require_connection_permission: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            host_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "GlyphRay Host".to_string()),
            default_display_id: 0,
            encoder_preference: EncoderPreference::Auto,
            require_connection_permission: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderPreference {
    Auto,
    NvidiaNvenc,
    IntelQuickSync,
    AmdAmf,
    Software,
}

