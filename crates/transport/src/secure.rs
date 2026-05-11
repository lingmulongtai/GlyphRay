use crate::TransportError;
use glyphray_security::{ReplayGuard, SealedPacket, SecurityError, SessionCipher};

#[derive(Debug)]
pub struct SecureDatagramCodec {
    send_counter: u64,
    receive_guard: ReplayGuard,
    aad: Vec<u8>,
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
}
