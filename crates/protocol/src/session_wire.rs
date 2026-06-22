use sha2::{Digest, Sha256};

const MAGIC: [u8; 4] = *b"GLYH";
const VERSION: u16 = 1;
const SERVER_EXCHANGE: u8 = 1;
const CLIENT_CONFIRM: u8 = 2;
const HEADER_LEN: usize = 8;
const MAX_FIELD_LEN: usize = 4_096;
const SERVER_DOMAIN: &[u8] = b"GlyphRay server key exchange v1";
const CLIENT_DOMAIN: &[u8] = b"GlyphRay client key confirm v1";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionWireError {
    #[error("session handshake packet is too short")]
    ShortPacket,
    #[error("session handshake packet has invalid magic")]
    InvalidMagic,
    #[error("unsupported session handshake version {0}")]
    UnsupportedVersion(u16),
    #[error("unexpected session handshake message type")]
    UnexpectedType,
    #[error("session handshake field is too large")]
    FieldTooLarge,
    #[error("session handshake packet length is invalid")]
    InvalidLength,
    #[error("session handshake text is not UTF-8")]
    InvalidText,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerKeyExchange {
    pub session_id: [u8; 16],
    pub expires_at_unix_ms: u64,
    pub salt: [u8; 32],
    pub ephemeral_public_key_der: Vec<u8>,
    pub host_identity_public_key_der: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientKeyConfirm {
    pub session_id: [u8; 16],
    pub device_id: String,
    pub ephemeral_public_key_der: Vec<u8>,
    pub signature: Vec<u8>,
}

pub fn encode_server_key_exchange(
    exchange: &ServerKeyExchange,
) -> Result<Vec<u8>, SessionWireError> {
    validate_fields(&[
        &exchange.ephemeral_public_key_der,
        &exchange.host_identity_public_key_der,
        &exchange.signature,
    ])?;
    let mut out = header(SERVER_EXCHANGE);
    out.extend_from_slice(&exchange.session_id);
    out.extend_from_slice(&exchange.expires_at_unix_ms.to_le_bytes());
    out.extend_from_slice(&exchange.salt);
    put_bytes(&mut out, &exchange.ephemeral_public_key_der);
    put_bytes(&mut out, &exchange.host_identity_public_key_der);
    put_bytes(&mut out, &exchange.signature);
    Ok(out)
}

pub fn decode_server_key_exchange(bytes: &[u8]) -> Result<ServerKeyExchange, SessionWireError> {
    let mut cursor = check_header(bytes, SERVER_EXCHANGE)?;
    let session_id = take_array::<16>(bytes, &mut cursor)?;
    let expires_at_unix_ms = u64::from_le_bytes(take_array::<8>(bytes, &mut cursor)?);
    let salt = take_array::<32>(bytes, &mut cursor)?;
    let ephemeral_public_key_der = take_bytes(bytes, &mut cursor)?;
    let host_identity_public_key_der = take_bytes(bytes, &mut cursor)?;
    let signature = take_bytes(bytes, &mut cursor)?;
    finish(bytes, cursor)?;
    Ok(ServerKeyExchange {
        session_id,
        expires_at_unix_ms,
        salt,
        ephemeral_public_key_der,
        host_identity_public_key_der,
        signature,
    })
}

pub fn encode_client_key_confirm(confirm: &ClientKeyConfirm) -> Result<Vec<u8>, SessionWireError> {
    validate_fields(&[
        confirm.device_id.as_bytes(),
        &confirm.ephemeral_public_key_der,
        &confirm.signature,
    ])?;
    let mut out = header(CLIENT_CONFIRM);
    out.extend_from_slice(&confirm.session_id);
    put_bytes(&mut out, confirm.device_id.as_bytes());
    put_bytes(&mut out, &confirm.ephemeral_public_key_der);
    put_bytes(&mut out, &confirm.signature);
    Ok(out)
}

pub fn decode_client_key_confirm(bytes: &[u8]) -> Result<ClientKeyConfirm, SessionWireError> {
    let mut cursor = check_header(bytes, CLIENT_CONFIRM)?;
    let session_id = take_array::<16>(bytes, &mut cursor)?;
    let device_id = String::from_utf8(take_bytes(bytes, &mut cursor)?)
        .map_err(|_| SessionWireError::InvalidText)?;
    let ephemeral_public_key_der = take_bytes(bytes, &mut cursor)?;
    let signature = take_bytes(bytes, &mut cursor)?;
    finish(bytes, cursor)?;
    Ok(ClientKeyConfirm {
        session_id,
        device_id,
        ephemeral_public_key_der,
        signature,
    })
}

pub fn server_signing_payload(exchange: &ServerKeyExchange) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(SERVER_DOMAIN);
    out.extend_from_slice(&exchange.session_id);
    out.extend_from_slice(&exchange.expires_at_unix_ms.to_le_bytes());
    out.extend_from_slice(&exchange.salt);
    put_bytes(&mut out, &exchange.ephemeral_public_key_der);
    out
}

pub fn server_transcript_hash(exchange: &ServerKeyExchange) -> [u8; 32] {
    Sha256::digest(server_signing_payload(exchange)).into()
}

pub fn client_signing_payload(
    server_transcript_hash: &[u8; 32],
    confirm: &ClientKeyConfirm,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(CLIENT_DOMAIN);
    out.extend_from_slice(server_transcript_hash);
    out.extend_from_slice(&confirm.session_id);
    put_bytes(&mut out, confirm.device_id.as_bytes());
    put_bytes(&mut out, &confirm.ephemeral_public_key_der);
    out
}

pub fn session_transcript_hash(
    exchange: &ServerKeyExchange,
    confirm: &ClientKeyConfirm,
) -> [u8; 32] {
    let server_hash = server_transcript_hash(exchange);
    let client_payload = client_signing_payload(&server_hash, confirm);
    let mut hasher = Sha256::new();
    hasher.update(server_signing_payload(exchange));
    hasher.update(client_payload);
    hasher.finalize().into()
}

fn header(message_type: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.push(message_type);
    out.push(0);
    out
}

fn check_header(bytes: &[u8], expected_type: u8) -> Result<usize, SessionWireError> {
    if bytes.len() < HEADER_LEN {
        return Err(SessionWireError::ShortPacket);
    }
    if bytes[0..4] != MAGIC {
        return Err(SessionWireError::InvalidMagic);
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != VERSION {
        return Err(SessionWireError::UnsupportedVersion(version));
    }
    if bytes[6] != expected_type {
        return Err(SessionWireError::UnexpectedType);
    }
    Ok(HEADER_LEN)
}

fn put_bytes(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(&(value.len() as u16).to_le_bytes());
    out.extend_from_slice(value);
}

fn take_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Vec<u8>, SessionWireError> {
    let len = u16::from_le_bytes(take_array::<2>(bytes, cursor)?) as usize;
    if len > MAX_FIELD_LEN || bytes.len().saturating_sub(*cursor) < len {
        return Err(SessionWireError::InvalidLength);
    }
    let value = bytes[*cursor..*cursor + len].to_vec();
    *cursor += len;
    Ok(value)
}

fn take_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], SessionWireError> {
    if bytes.len().saturating_sub(*cursor) < N {
        return Err(SessionWireError::InvalidLength);
    }
    let value = bytes[*cursor..*cursor + N]
        .try_into()
        .expect("slice length checked");
    *cursor += N;
    Ok(value)
}

fn validate_fields(fields: &[&[u8]]) -> Result<(), SessionWireError> {
    if fields.iter().any(|field| field.len() > MAX_FIELD_LEN) {
        Err(SessionWireError::FieldTooLarge)
    } else {
        Ok(())
    }
}

fn finish(bytes: &[u8], cursor: usize) -> Result<(), SessionWireError> {
    if cursor == bytes.len() {
        Ok(())
    } else {
        Err(SessionWireError::InvalidLength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_exchange_messages_round_trip() {
        let server = ServerKeyExchange {
            session_id: [1; 16],
            expires_at_unix_ms: 42,
            salt: [2; 32],
            ephemeral_public_key_der: vec![3; 91],
            host_identity_public_key_der: vec![4; 91],
            signature: vec![5; 70],
        };
        assert_eq!(
            decode_server_key_exchange(&encode_server_key_exchange(&server).expect("encode"))
                .expect("decode"),
            server
        );

        let client = ClientKeyConfirm {
            session_id: [6; 16],
            device_id: "trusted-device".to_string(),
            ephemeral_public_key_der: vec![7; 91],
            signature: vec![8; 70],
        };
        assert_eq!(
            decode_client_key_confirm(&encode_client_key_confirm(&client).expect("encode"))
                .expect("decode"),
            client
        );
    }
}
