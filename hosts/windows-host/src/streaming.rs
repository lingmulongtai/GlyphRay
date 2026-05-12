use crate::capture::{CaptureError, ScreenCapture};
use crate::encoder::{EncoderError, VideoEncoder};
use glyphray_transport::video::{EncodedVideoAccessUnit, VideoPacketizer};
use glyphray_transport::{RealtimeTransport, TransportError, TransportPacket};

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Encode(#[from] EncoderError),
    #[error(transparent)]
    Transport(#[from] TransportError),
}

pub struct VideoStreamPipeline<C, E, T> {
    capture: C,
    encoder: E,
    transport: T,
    packetizer: VideoPacketizer,
    display_id: u32,
}

impl<C, E, T> VideoStreamPipeline<C, E, T>
where
    C: ScreenCapture,
    E: VideoEncoder,
    T: RealtimeTransport,
{
    pub fn new(
        capture: C,
        encoder: E,
        transport: T,
        packetizer: VideoPacketizer,
        display_id: u32,
    ) -> Self {
        Self {
            capture,
            encoder,
            transport,
            packetizer,
            display_id,
        }
    }

    pub fn start(&mut self) -> Result<(), StreamError> {
        self.encoder.start()?;
        Ok(())
    }

    pub fn capture_encode_packetize(&mut self) -> Result<Vec<TransportPacket>, StreamError> {
        let frame = self.capture.capture_frame(self.display_id)?;
        let encoded = self.encoder.encode(&frame)?;
        let access_unit = EncodedVideoAccessUnit {
            sequence: encoded.sequence,
            codec: encoded.codec,
            presentation_time_us: encoded.capture_timestamp_us,
            is_keyframe: encoded.is_keyframe,
            payload: encoded.payload,
        };

        Ok(self.packetizer.packetize(&access_unit)?)
    }

    pub fn send_one_frame(&mut self) -> Result<usize, StreamError> {
        let packets = self.capture_encode_packetize()?;
        let packet_count = packets.len();
        for packet in packets {
            self.transport.send(packet)?;
        }
        Ok(packet_count)
    }
}
