use crate::{StylusAction, StylusInputBatch, StylusSample, StylusToolType};

const STYLUS_MAGIC: [u8; 4] = *b"GLYS";
const STYLUS_WIRE_VERSION: u16 = 1;
const HEADER_LEN: usize = 28;
const SAMPLE_LEN: usize = 58;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StylusWireError {
    #[error("stylus packet is shorter than the header")]
    ShortPacket,
    #[error("invalid stylus packet magic")]
    InvalidMagic,
    #[error("unsupported stylus packet version {0}")]
    UnsupportedVersion(u16),
    #[error("stylus packet length mismatch")]
    LengthMismatch,
    #[error("unknown stylus tool type {0}")]
    UnknownToolType(u8),
    #[error("unknown stylus action {0}")]
    UnknownAction(u8),
    #[error("too many stylus samples in one packet")]
    TooManySamples,
}

pub fn encode_stylus_batch(batch: &StylusInputBatch) -> Result<Vec<u8>, StylusWireError> {
    if batch.samples.len() > u16::MAX as usize {
        return Err(StylusWireError::TooManySamples);
    }

    let mut out = Vec::with_capacity(HEADER_LEN + SAMPLE_LEN * batch.samples.len());
    out.extend_from_slice(&STYLUS_MAGIC);
    out.extend_from_slice(&STYLUS_WIRE_VERSION.to_le_bytes());
    out.extend_from_slice(&batch.batch_sequence.to_le_bytes());
    out.extend_from_slice(&batch.monotonic_timestamp_us.to_le_bytes());
    out.extend_from_slice(&(batch.samples.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0_u8; 4]);

    for sample in &batch.samples {
        out.extend_from_slice(&sample.sequence.to_le_bytes());
        out.extend_from_slice(&sample.timestamp_us.to_le_bytes());
        out.extend_from_slice(&sample.display_id.to_le_bytes());
        out.extend_from_slice(&sample.pointer_id.to_le_bytes());
        out.push(tool_to_u8(sample.tool_type));
        out.push(action_to_u8(sample.action));
        out.extend_from_slice(&sample.x.to_le_bytes());
        out.extend_from_slice(&sample.y.to_le_bytes());
        out.extend_from_slice(&sample.pressure.to_le_bytes());
        out.extend_from_slice(&sample.tilt_x_degrees.to_le_bytes());
        out.extend_from_slice(&sample.tilt_y_degrees.to_le_bytes());
        out.extend_from_slice(&sample.orientation_degrees.to_le_bytes());
        out.extend_from_slice(&sample.button_flags.to_le_bytes());
        let flags = u8::from(sample.hover)
            | (u8::from(sample.eraser) << 1)
            | (u8::from(sample.predicted) << 2);
        out.push(flags);
        out.push(0);
        out.push(0);
        out.push(0);
    }

    Ok(out)
}

pub fn decode_stylus_batch(bytes: &[u8]) -> Result<StylusInputBatch, StylusWireError> {
    if bytes.len() < HEADER_LEN {
        return Err(StylusWireError::ShortPacket);
    }
    if bytes[0..4] != STYLUS_MAGIC[..] {
        return Err(StylusWireError::InvalidMagic);
    }

    let version = u16::from_le_bytes(bytes[4..6].try_into().expect("slice length"));
    if version != STYLUS_WIRE_VERSION {
        return Err(StylusWireError::UnsupportedVersion(version));
    }

    let batch_sequence = u64::from_le_bytes(bytes[6..14].try_into().expect("slice length"));
    let monotonic_timestamp_us =
        u64::from_le_bytes(bytes[14..22].try_into().expect("slice length"));
    let sample_count = u16::from_le_bytes(bytes[22..24].try_into().expect("slice length")) as usize;
    if bytes.len() != HEADER_LEN + sample_count * SAMPLE_LEN {
        return Err(StylusWireError::LengthMismatch);
    }

    let mut samples = Vec::with_capacity(sample_count);
    let mut offset = HEADER_LEN;
    for _ in 0..sample_count {
        let sequence = u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("slice"));
        offset += 8;
        let timestamp_us = u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("slice"));
        offset += 8;
        let display_id = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let pointer_id = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let tool_type = tool_from_u8(bytes[offset])?;
        offset += 1;
        let action = action_from_u8(bytes[offset])?;
        offset += 1;
        let x = f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let y = f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let pressure = f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let tilt_x_degrees = f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let tilt_y_degrees = f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let orientation_degrees =
            f32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let button_flags = u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("slice"));
        offset += 4;
        let flags = bytes[offset];
        offset += 4;

        samples.push(StylusSample {
            sequence,
            timestamp_us,
            display_id,
            pointer_id,
            tool_type,
            action,
            x,
            y,
            pressure,
            tilt_x_degrees,
            tilt_y_degrees,
            orientation_degrees,
            button_flags,
            hover: (flags & 0b001) != 0,
            eraser: (flags & 0b010) != 0,
            predicted: (flags & 0b100) != 0,
        });
    }

    Ok(StylusInputBatch {
        batch_sequence,
        monotonic_timestamp_us,
        samples,
    })
}

fn tool_to_u8(tool_type: StylusToolType) -> u8 {
    match tool_type {
        StylusToolType::Unknown => 0,
        StylusToolType::Finger => 1,
        StylusToolType::Stylus => 2,
        StylusToolType::Eraser => 3,
        StylusToolType::Mouse => 4,
    }
}

fn tool_from_u8(value: u8) -> Result<StylusToolType, StylusWireError> {
    match value {
        0 => Ok(StylusToolType::Unknown),
        1 => Ok(StylusToolType::Finger),
        2 => Ok(StylusToolType::Stylus),
        3 => Ok(StylusToolType::Eraser),
        4 => Ok(StylusToolType::Mouse),
        _ => Err(StylusWireError::UnknownToolType(value)),
    }
}

fn action_to_u8(action: StylusAction) -> u8 {
    match action {
        StylusAction::HoverEnter => 0,
        StylusAction::HoverMove => 1,
        StylusAction::HoverExit => 2,
        StylusAction::Down => 3,
        StylusAction::Move => 4,
        StylusAction::Up => 5,
        StylusAction::Cancel => 6,
    }
}

fn action_from_u8(value: u8) -> Result<StylusAction, StylusWireError> {
    match value {
        0 => Ok(StylusAction::HoverEnter),
        1 => Ok(StylusAction::HoverMove),
        2 => Ok(StylusAction::HoverExit),
        3 => Ok(StylusAction::Down),
        4 => Ok(StylusAction::Move),
        5 => Ok(StylusAction::Up),
        6 => Ok(StylusAction::Cancel),
        _ => Err(StylusWireError::UnknownAction(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_stylus_wire_round_trips() {
        let batch = StylusInputBatch {
            batch_sequence: 11,
            monotonic_timestamp_us: 99,
            samples: vec![StylusSample {
                sequence: 1,
                timestamp_us: 2,
                display_id: 3,
                pointer_id: 4,
                tool_type: StylusToolType::Stylus,
                action: StylusAction::Move,
                x: 12.5,
                y: 44.0,
                pressure: 0.7,
                tilt_x_degrees: 9.0,
                tilt_y_degrees: -10.0,
                orientation_degrees: 35.0,
                button_flags: 2,
                hover: false,
                eraser: false,
                predicted: true,
            }],
        };

        let decoded = decode_stylus_batch(&encode_stylus_batch(&batch).expect("encode"))
            .expect("decode");
        assert_eq!(decoded, batch);
    }
}
