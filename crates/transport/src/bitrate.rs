use crate::ConnectionStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveBitrateConfig {
    pub min_kbps: u32,
    pub max_kbps: u32,
    pub initial_kbps: u32,
    pub decrease_loss_percent: u32,
    pub increase_loss_percent: u32,
}

impl Default for AdaptiveBitrateConfig {
    fn default() -> Self {
        Self {
            min_kbps: 4_000,
            max_kbps: 80_000,
            initial_kbps: 18_000,
            decrease_loss_percent: 4,
            increase_loss_percent: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveBitrateController {
    config: AdaptiveBitrateConfig,
    current_kbps: u32,
}

impl AdaptiveBitrateController {
    pub fn new(config: AdaptiveBitrateConfig) -> Self {
        let current_kbps = config.initial_kbps.clamp(config.min_kbps, config.max_kbps);
        Self {
            config,
            current_kbps,
        }
    }

    pub fn current_kbps(&self) -> u32 {
        self.current_kbps
    }

    pub fn update(&mut self, stats: ConnectionStats) -> u32 {
        let loss = stats.packet_loss_percent.max(0.0) as u32;
        let next = if loss >= self.config.decrease_loss_percent || stats.jitter_ms > 12.0 {
            (self.current_kbps as f32 * 0.82).round() as u32
        } else if loss <= self.config.increase_loss_percent && stats.jitter_ms < 5.0 {
            (self.current_kbps as f32 * 1.06).round() as u32
        } else {
            self.current_kbps
        };
        self.current_kbps = next.clamp(self.config.min_kbps, self.config.max_kbps);
        self.current_kbps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrate_reduces_under_loss() {
        let mut controller = AdaptiveBitrateController::new(AdaptiveBitrateConfig::default());
        let before = controller.current_kbps();
        let after = controller.update(ConnectionStats {
            rtt_ms: 8.0,
            jitter_ms: 3.0,
            packet_loss_percent: 8.0,
            estimated_bandwidth_kbps: 20_000,
        });
        assert!(after < before);
    }

    #[test]
    fn bitrate_increases_under_clean_network() {
        let mut controller = AdaptiveBitrateController::new(AdaptiveBitrateConfig::default());
        let before = controller.current_kbps();
        let after = controller.update(ConnectionStats {
            rtt_ms: 4.0,
            jitter_ms: 1.0,
            packet_loss_percent: 0.0,
            estimated_bandwidth_kbps: 80_000,
        });
        assert!(after > before);
    }
}
