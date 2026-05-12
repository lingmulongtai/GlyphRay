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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkAdaptationDecision {
    pub target_bitrate_kbps: u32,
    pub request_keyframe: bool,
    pub reason: AdaptationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptationReason {
    Stable,
    CleanNetwork,
    PacketLoss,
    HighJitter,
    BandwidthLimited,
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
        self.update_with_decision(stats).target_bitrate_kbps
    }

    pub fn update_with_decision(&mut self, stats: ConnectionStats) -> NetworkAdaptationDecision {
        let loss = stats.packet_loss_percent.max(0.0);
        let estimated_ceiling = bandwidth_ceiling(stats.estimated_bandwidth_kbps);
        let bandwidth_limited = estimated_ceiling
            .map(|ceiling| ceiling < self.current_kbps)
            .unwrap_or(false);
        let (next, reason, request_keyframe) = if loss >= self.config.decrease_loss_percent as f32 {
            (
                (self.current_kbps as f32 * 0.82).round() as u32,
                AdaptationReason::PacketLoss,
                loss >= (self.config.decrease_loss_percent * 2) as f32,
            )
        } else if stats.jitter_ms > 12.0 {
            (
                (self.current_kbps as f32 * 0.82).round() as u32,
                AdaptationReason::HighJitter,
                false,
            )
        } else if bandwidth_limited {
            (
                estimated_ceiling.expect("checked above"),
                AdaptationReason::BandwidthLimited,
                false,
            )
        } else if loss <= self.config.increase_loss_percent as f32 && stats.jitter_ms < 5.0 {
            (
                (self.current_kbps as f32 * 1.06).round() as u32,
                AdaptationReason::CleanNetwork,
                false,
            )
        } else {
            (self.current_kbps, AdaptationReason::Stable, false)
        };

        self.current_kbps = estimated_ceiling
            .map(|ceiling| next.min(ceiling))
            .unwrap_or(next)
            .clamp(self.config.min_kbps, self.config.max_kbps);

        NetworkAdaptationDecision {
            target_bitrate_kbps: self.current_kbps,
            request_keyframe,
            reason,
        }
    }
}

fn bandwidth_ceiling(estimated_bandwidth_kbps: u32) -> Option<u32> {
    if estimated_bandwidth_kbps == 0 {
        return None;
    }
    Some(((estimated_bandwidth_kbps as f32) * 0.9).round() as u32)
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

    #[test]
    fn severe_loss_requests_keyframe_recovery() {
        let mut controller = AdaptiveBitrateController::new(AdaptiveBitrateConfig::default());
        let decision = controller.update_with_decision(ConnectionStats {
            rtt_ms: 12.0,
            jitter_ms: 4.0,
            packet_loss_percent: 9.0,
            estimated_bandwidth_kbps: 20_000,
        });

        assert_eq!(decision.reason, AdaptationReason::PacketLoss);
        assert!(decision.request_keyframe);
        assert!(decision.target_bitrate_kbps < AdaptiveBitrateConfig::default().initial_kbps);
    }

    #[test]
    fn estimated_bandwidth_caps_target_bitrate() {
        let mut controller = AdaptiveBitrateController::new(AdaptiveBitrateConfig::default());
        let decision = controller.update_with_decision(ConnectionStats {
            rtt_ms: 6.0,
            jitter_ms: 2.0,
            packet_loss_percent: 0.0,
            estimated_bandwidth_kbps: 10_000,
        });

        assert_eq!(decision.reason, AdaptationReason::BandwidthLimited);
        assert_eq!(decision.target_bitrate_kbps, 9_000);
        assert!(!decision.request_keyframe);
    }
}
