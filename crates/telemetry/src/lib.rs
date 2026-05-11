use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MetricKind {
    EncodeTimeUs,
    NetworkTimeUs,
    DecodeTimeUs,
    RenderTimeUs,
    InputCaptureUs,
    InputTransportUs,
    InputInjectionUs,
    EndToEndEstimateUs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricSample {
    pub kind: MetricKind,
    pub timestamp_us: u64,
    pub value_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    pub encode_us: u64,
    pub network_us: u64,
    pub decode_us: u64,
    pub render_us: u64,
    pub input_capture_us: u64,
    pub input_transport_us: u64,
    pub input_injection_us: u64,
}

impl LatencyBreakdown {
    pub fn video_total_us(&self) -> u64 {
        self.encode_us + self.network_us + self.decode_us + self.render_us
    }

    pub fn input_total_us(&self) -> u64 {
        self.input_capture_us + self.input_transport_us + self.input_injection_us
    }
}

#[derive(Debug, Clone)]
pub struct RollingWindow {
    max_samples: usize,
    samples: VecDeque<u64>,
}

impl RollingWindow {
    pub fn new(max_samples: usize) -> Self {
        Self {
            max_samples: max_samples.max(1),
            samples: VecDeque::new(),
        }
    }

    pub fn push(&mut self, value: u64) {
        self.samples.push_back(value);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }
    }

    pub fn p95(&self) -> Option<u64> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sorted: Vec<u64> = self.samples.iter().copied().collect();
        sorted.sort_unstable();
        let index = ((sorted.len() as f32 - 1.0) * 0.95).round() as usize;
        sorted.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_breakdown_totals_are_separate_for_video_and_input() {
        let breakdown = LatencyBreakdown {
            encode_us: 1,
            network_us: 2,
            decode_us: 3,
            render_us: 4,
            input_capture_us: 5,
            input_transport_us: 6,
            input_injection_us: 7,
        };

        assert_eq!(breakdown.video_total_us(), 10);
        assert_eq!(breakdown.input_total_us(), 18);
    }

    #[test]
    fn rolling_window_computes_p95() {
        let mut window = RollingWindow::new(100);
        for value in 1..=100 {
            window.push(value);
        }
        assert_eq!(window.p95(), Some(95));
    }
}

