use crate::TransportError;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};

const DISCOVERY_MAGIC: [u8; 4] = *b"GLYD";
const DISCOVERY_VERSION: u16 = 1;
const HEADER_LEN: usize = 33;
const MAX_HOST_NAME_LEN: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostAdvertisement {
    pub host_id: [u8; 16],
    pub host_name: String,
    pub protocol_version: u16,
    pub control_port: u16,
    pub video_port: u16,
    pub supports_windows_ink: bool,
    pub supports_h264: bool,
    pub pairing_required: bool,
    pub load_percent: u8,
}

impl HostAdvertisement {
    pub fn encode(&self) -> Result<Vec<u8>, TransportError> {
        let name = self.host_name.as_bytes();
        if name.len() > MAX_HOST_NAME_LEN {
            return Err(TransportError::PayloadTooLarge);
        }

        let mut out = Vec::with_capacity(HEADER_LEN + name.len());
        out.extend_from_slice(&DISCOVERY_MAGIC);
        out.extend_from_slice(&DISCOVERY_VERSION.to_le_bytes());
        out.extend_from_slice(&self.host_id);
        out.extend_from_slice(&self.protocol_version.to_le_bytes());
        out.extend_from_slice(&self.control_port.to_le_bytes());
        out.extend_from_slice(&self.video_port.to_le_bytes());
        out.push(flags(self));
        out.push(self.load_percent.min(100));
        out.push(name.len() as u8);
        out.extend_from_slice(&[0_u8; 2]);
        out.extend_from_slice(name);
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TransportError> {
        if bytes.len() < HEADER_LEN {
            return Err(TransportError::Decode(
                "short discovery advertisement".to_string(),
            ));
        }
        if bytes[0..4] != DISCOVERY_MAGIC[..] {
            return Err(TransportError::Decode(
                "bad discovery advertisement magic".to_string(),
            ));
        }
        let version = u16::from_le_bytes(bytes[4..6].try_into().expect("slice length"));
        if version != DISCOVERY_VERSION {
            return Err(TransportError::Decode(format!(
                "unsupported discovery version {version}"
            )));
        }

        let mut host_id = [0_u8; 16];
        host_id.copy_from_slice(&bytes[6..22]);
        let protocol_version = u16::from_le_bytes(bytes[22..24].try_into().expect("slice length"));
        let control_port = u16::from_le_bytes(bytes[24..26].try_into().expect("slice length"));
        let video_port = u16::from_le_bytes(bytes[26..28].try_into().expect("slice length"));
        let raw_flags = bytes[28];
        let load_percent = bytes[29].min(100);
        let name_len = bytes[30] as usize;
        if bytes.len() != HEADER_LEN + name_len {
            return Err(TransportError::Decode(
                "discovery advertisement length mismatch".to_string(),
            ));
        }
        let host_name = String::from_utf8(bytes[HEADER_LEN..].to_vec())
            .map_err(|_| TransportError::Decode("host name is not utf-8".to_string()))?;

        Ok(Self {
            host_id,
            host_name,
            protocol_version,
            control_port,
            video_port,
            supports_windows_ink: (raw_flags & 0b0000_0001) != 0,
            supports_h264: (raw_flags & 0b0000_0010) != 0,
            pairing_required: (raw_flags & 0b0000_0100) != 0,
            load_percent,
        })
    }
}

pub struct LanDiscoverySocket {
    socket: UdpSocket,
    broadcast_addr: SocketAddr,
}

impl LanDiscoverySocket {
    pub fn bind(port: u16) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
            .map_err(io_error)?;
        socket.set_broadcast(true).map_err(io_error)?;
        socket.set_nonblocking(true).map_err(io_error)?;
        Ok(Self {
            socket,
            broadcast_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::BROADCAST, port)),
        })
    }

    pub fn announce(&self, advertisement: &HostAdvertisement) -> Result<usize, TransportError> {
        let payload = advertisement.encode()?;
        self.socket.send_to(&payload, self.broadcast_addr).map_err(io_error)
    }

    pub fn poll(&self) -> Result<Option<(HostAdvertisement, SocketAddr)>, TransportError> {
        let mut buffer = [0_u8; 512];
        match self.socket.recv_from(&mut buffer) {
            Ok((len, peer)) => HostAdvertisement::decode(&buffer[..len]).map(|ad| Some((ad, peer))),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(io_error(err)),
        }
    }
}

fn flags(advertisement: &HostAdvertisement) -> u8 {
    u8::from(advertisement.supports_windows_ink)
        | (u8::from(advertisement.supports_h264) << 1)
        | (u8::from(advertisement.pairing_required) << 2)
}

fn io_error(error: std::io::Error) -> TransportError {
    TransportError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_advertisement_round_trips() {
        let ad = HostAdvertisement {
            host_id: [7_u8; 16],
            host_name: "Studio Host".to_string(),
            protocol_version: 1,
            control_port: 44000,
            video_port: 44001,
            supports_windows_ink: true,
            supports_h264: true,
            pairing_required: true,
            load_percent: 12,
        };

        let decoded = HostAdvertisement::decode(&ad.encode().expect("encode")).expect("decode");
        assert_eq!(decoded, ad);
    }

    #[test]
    fn discovery_rejects_oversized_name() {
        let ad = HostAdvertisement {
            host_id: [0_u8; 16],
            host_name: "x".repeat(300),
            protocol_version: 1,
            control_port: 1,
            video_port: 2,
            supports_windows_ink: false,
            supports_h264: false,
            pairing_required: false,
            load_percent: 0,
        };

        assert!(matches!(ad.encode(), Err(TransportError::PayloadTooLarge)));
    }
}

