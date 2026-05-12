use crate::TransportError;
use std::collections::BTreeMap;

const FRAGMENT_MAGIC: [u8; 4] = *b"GLYF";
const FRAGMENT_HEADER_LEN: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameFragment {
    pub frame_sequence: u64,
    pub fragment_index: u16,
    pub fragment_count: u16,
    pub payload: Vec<u8>,
}

pub fn fragment_frame(
    frame_sequence: u64,
    payload: &[u8],
    max_fragment_payload: usize,
) -> Result<Vec<FrameFragment>, TransportError> {
    if max_fragment_payload == 0 {
        return Err(TransportError::PayloadTooLarge);
    }

    let fragment_count = payload.len().div_ceil(max_fragment_payload);
    if fragment_count > u16::MAX as usize {
        return Err(TransportError::PayloadTooLarge);
    }

    let mut fragments = Vec::with_capacity(fragment_count.max(1));
    if payload.is_empty() {
        fragments.push(FrameFragment {
            frame_sequence,
            fragment_index: 0,
            fragment_count: 1,
            payload: Vec::new(),
        });
        return Ok(fragments);
    }

    for (index, chunk) in payload.chunks(max_fragment_payload).enumerate() {
        fragments.push(FrameFragment {
            frame_sequence,
            fragment_index: index as u16,
            fragment_count: fragment_count as u16,
            payload: chunk.to_vec(),
        });
    }

    Ok(fragments)
}

pub fn encode_fragment(fragment: &FrameFragment) -> Result<Vec<u8>, TransportError> {
    let mut out = Vec::with_capacity(FRAGMENT_HEADER_LEN + fragment.payload.len());
    out.extend_from_slice(&FRAGMENT_MAGIC);
    out.extend_from_slice(&fragment.frame_sequence.to_le_bytes());
    out.extend_from_slice(&fragment.fragment_index.to_le_bytes());
    out.extend_from_slice(&fragment.fragment_count.to_le_bytes());
    out.extend_from_slice(&(fragment.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&fragment.payload);
    Ok(out)
}

pub fn decode_fragment(bytes: &[u8]) -> Result<FrameFragment, TransportError> {
    if bytes.len() < FRAGMENT_HEADER_LEN {
        return Err(TransportError::Decode("short video fragment".to_string()));
    }
    if bytes[0..4] != FRAGMENT_MAGIC[..] {
        return Err(TransportError::Decode(
            "bad video fragment magic".to_string(),
        ));
    }

    let frame_sequence = u64::from_le_bytes(bytes[4..12].try_into().expect("slice length"));
    let fragment_index = u16::from_le_bytes(bytes[12..14].try_into().expect("slice length"));
    let fragment_count = u16::from_le_bytes(bytes[14..16].try_into().expect("slice length"));
    let payload_len = u32::from_le_bytes(bytes[16..20].try_into().expect("slice length")) as usize;

    if fragment_count == 0 || fragment_index >= fragment_count {
        return Err(TransportError::Decode(
            "invalid video fragment index".to_string(),
        ));
    }
    if bytes.len() != FRAGMENT_HEADER_LEN + payload_len {
        return Err(TransportError::Decode(
            "video fragment payload length mismatch".to_string(),
        ));
    }

    Ok(FrameFragment {
        frame_sequence,
        fragment_index,
        fragment_count,
        payload: bytes[FRAGMENT_HEADER_LEN..].to_vec(),
    })
}

#[derive(Debug, Default)]
pub struct FrameReassembler {
    pending: BTreeMap<u64, PendingFrame>,
}

impl FrameReassembler {
    pub fn push(&mut self, fragment: FrameFragment) -> Result<Option<Vec<u8>>, TransportError> {
        let frame_sequence = fragment.frame_sequence;
        let fragment_index = fragment.fragment_index;
        let fragment_count = fragment.fragment_count;
        let payload = fragment.payload;
        if fragment_count == 0 || fragment_index >= fragment_count {
            return Err(TransportError::Decode(
                "invalid video fragment index".to_string(),
            ));
        }

        let entry = self
            .pending
            .entry(frame_sequence)
            .or_insert_with(|| PendingFrame::new(fragment_count));

        if entry.fragment_count != fragment_count {
            return Err(TransportError::Decode(
                "fragment count changed within a frame".to_string(),
            ));
        }

        let index = fragment_index as usize;
        if entry.fragments[index].is_none() {
            entry.received += 1;
            entry.fragments[index] = Some(payload);
        }

        if entry.received == entry.fragment_count as usize {
            let complete = self
                .pending
                .remove(&frame_sequence)
                .expect("entry exists")
                .join();
            return Ok(Some(complete));
        }

        Ok(None)
    }
}

#[derive(Debug)]
struct PendingFrame {
    fragment_count: u16,
    received: usize,
    fragments: Vec<Option<Vec<u8>>>,
}

impl PendingFrame {
    fn new(fragment_count: u16) -> Self {
        Self {
            fragment_count,
            received: 0,
            fragments: vec![None; fragment_count as usize],
        }
    }

    fn join(self) -> Vec<u8> {
        let total_len: usize = self
            .fragments
            .iter()
            .filter_map(|fragment| fragment.as_ref())
            .map(Vec::len)
            .sum();
        let mut out = Vec::with_capacity(total_len);
        for fragment in self.fragments.into_iter().flatten() {
            out.extend_from_slice(&fragment);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_payload_fragments_and_reassembles() {
        let payload: Vec<u8> = (0..150_000).map(|value| (value % 251) as u8).collect();
        let fragments = fragment_frame(42, &payload, 16_000).expect("fragment");
        assert!(fragments.len() > 1);

        let mut reassembler = FrameReassembler::default();
        let mut completed = None;
        for fragment in fragments {
            let encoded = encode_fragment(&fragment).expect("encode");
            let decoded = decode_fragment(&encoded).expect("decode");
            completed = reassembler.push(decoded).expect("reassemble");
        }

        assert_eq!(completed, Some(payload));
    }

    #[test]
    fn rejects_invalid_fragment_index() {
        let bad = FrameFragment {
            frame_sequence: 1,
            fragment_index: 2,
            fragment_count: 2,
            payload: vec![1],
        };

        let encoded = encode_fragment(&bad).expect("encode");
        assert!(matches!(
            decode_fragment(&encoded),
            Err(TransportError::Decode(_))
        ));
    }
}
