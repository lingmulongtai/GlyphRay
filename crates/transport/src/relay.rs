use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelayCandidateKind {
    DirectLan,
    StunReflexive,
    TurnRelay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayCandidate {
    pub kind: RelayCandidateKind,
    pub endpoint: String,
    pub estimated_rtt_ms: u32,
    pub trusted: bool,
}

pub fn select_best_candidate(candidates: &[RelayCandidate]) -> Option<RelayCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.trusted)
        .min_by_key(|candidate| {
            let kind_cost = match candidate.kind {
                RelayCandidateKind::DirectLan => 0,
                RelayCandidateKind::StunReflexive => 1_000,
                RelayCandidateKind::TurnRelay => 5_000,
            };
            kind_cost + candidate.estimated_rtt_ms
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_lan_candidate_is_preferred_when_trusted() {
        let selected = select_best_candidate(&[
            RelayCandidate {
                kind: RelayCandidateKind::TurnRelay,
                endpoint: "relay.example:443".to_string(),
                estimated_rtt_ms: 20,
                trusted: true,
            },
            RelayCandidate {
                kind: RelayCandidateKind::DirectLan,
                endpoint: "192.168.1.2:44000".to_string(),
                estimated_rtt_ms: 4,
                trusted: true,
            },
        ])
        .expect("candidate");

        assert_eq!(selected.kind, RelayCandidateKind::DirectLan);
    }
}

