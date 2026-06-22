use crate::TransportError;
use glyphray_security::{ReplayGuard, SealedPacket, SecurityError, SessionCipher};

const SECURE_MAGIC: [u8; 4] = *b"GLYE";
const SECURE_VERSION: u16 = 1;
const SECURE_HEADER_LEN: usize = 18;
const MAX_CIPHERTEXT_LEN: usize = 64 * 1024;

#[derive(Debug)]
pub struct SecureDatagramCodec {
    send_counter: u64,
    receive_guard: ReplayGuard,
    aad: Vec<u8>,
}

pub fn encode_sealed_datagram(packet: &SealedPacket) -> Result<Vec<u8>, TransportError> {
    if packet.ciphertext.len() > MAX_CIPHERTEXT_LEN {
        return Err(TransportError::PayloadTooLarge);
    }
    let mut out = Vec::with_capacity(SECURE_HEADER_LEN + packet.ciphertext.len());
    out.extend_from_slice(&SECURE_MAGIC);
    out.extend_from_slice(&SECURE_VERSION.to_le_bytes());
    out.extend_from_slice(&packet.counter.to_le_bytes());
    out.extend_from_slice(&(packet.ciphertext.len() as u32).to_le_bytes());
    out.extend_from_slice(&packet.ciphertext);
    Ok(out)
}

pub fn decode_sealed_datagram(bytes: &[u8]) -> Result<SealedPacket, TransportError> {
    if bytes.len() < SECURE_HEADER_LEN {
        return Err(TransportError::Decode("short secure datagram".to_string()));
    }
    if bytes[0..4] != SECURE_MAGIC {
        return Err(TransportError::Decode(
            "bad secure datagram magic".to_string(),
        ));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != SECURE_VERSION {
        return Err(TransportError::Decode(format!(
            "unsupported secure datagram version {version}"
        )));
    }
    let counter = u64::from_le_bytes(bytes[6..14].try_into().expect("slice length"));
    let ciphertext_len =
        u32::from_le_bytes(bytes[14..18].try_into().expect("slice length")) as usize;
    if ciphertext_len > MAX_CIPHERTEXT_LEN || bytes.len() != SECURE_HEADER_LEN + ciphertext_len {
        return Err(TransportError::Decode(
            "secure datagram length mismatch".to_string(),
        ));
    }
    Ok(SealedPacket {
        counter,
        ciphertext: bytes[SECURE_HEADER_LEN..].to_vec(),
    })
}

impl SecureDatagramCodec {
    pub fn new(aad: impl Into<Vec<u8>>) -> Self {
        Self {
            send_counter: 1,
            receive_guard: ReplayGuard::new(4_096),
            aad: aad.into(),
        }
    }

    pub fn seal(
        &mut self,
        cipher: &SessionCipher,
        plaintext_datagram: &[u8],
    ) -> Result<SealedPacket, TransportError> {
        let counter = self.send_counter;
        self.send_counter += 1;
        cipher
            .seal(counter, &self.aad, plaintext_datagram)
            .map_err(security_to_transport)
    }

    pub fn open(
        &mut self,
        cipher: &SessionCipher,
        packet: &SealedPacket,
    ) -> Result<Vec<u8>, TransportError> {
        let plaintext = cipher
            .open(packet, &self.aad)
            .map_err(security_to_transport)?;
        self.receive_guard
            .accept(packet.counter)
            .map_err(security_to_transport)?;
        Ok(plaintext)
    }
}

fn security_to_transport(error: SecurityError) -> TransportError {
    TransportError::Decode(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphray_security::{SecretBytes, SessionCipher};

    #[test]
    fn secure_datagram_codec_round_trips() {
        let secret = SecretBytes::from_bytes(b"transport session secret".to_vec());
        let cipher = SessionCipher::new(&secret, b"handshake");
        let mut sender = SecureDatagramCodec::new(b"glyphray".to_vec());
        let mut receiver = SecureDatagramCodec::new(b"glyphray".to_vec());

        let sealed = sender.seal(&cipher, b"datagram").expect("seal");
        let opened = receiver.open(&cipher, &sealed).expect("open");
        assert_eq!(opened, b"datagram");
    }

    #[test]
    fn sealed_datagram_wire_round_trips() {
        let packet = SealedPacket {
            counter: 99,
            ciphertext: vec![1, 2, 3, 4],
        };
        let encoded = encode_sealed_datagram(&packet).expect("encode");
        assert_eq!(&encoded[0..4], b"GLYE");
        assert_eq!(decode_sealed_datagram(&encoded).expect("decode"), packet);
    }
}
