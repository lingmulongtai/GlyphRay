use crate::audio::{validate_audio_frame, AudioCapture, AudioCaptureError};
use crate::capture::{CaptureError, ScreenCapture};
use crate::encoder::{EncodedVideoFrame, EncoderError, VideoEncoder};
use glyphray_audio::{AudioError, AudioPacketizer};
use glyphray_protocol::{encode_frame, Message, MessageKind, ProtocolError};
use glyphray_transport::video::{EncodedVideoAccessUnit, VideoPacketizer};
use glyphray_transport::{ChannelKind, RealtimeTransport, TransportError, TransportPacket};

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Encode(#[from] EncoderError),
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    AudioCapture(#[from] AudioCaptureError),
    #[error(transparent)]
    AudioPacketize(#[from] AudioError),
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
}

pub struct VideoStreamPipeline<C, E, T> {
    capture: C,
    encoder: E,
    transport: T,
    packetizer: VideoPacketizer,
    display_id: u32,
}

pub struct VideoPacketPipeline<C, E> {
    capture: C,
    encoder: E,
    packetizer: VideoPacketizer,
    display_id: u32,
}

pub struct AudioPacketPipeline<C> {
    capture: C,
    packetizer: AudioPacketizer,
    next_transport_sequence: u64,
}

impl<C, E> VideoPacketPipeline<C, E>
where
    C: ScreenCapture,
    E: VideoEncoder,
{
    pub fn new(capture: C, encoder: E, packetizer: VideoPacketizer, display_id: u32) -> Self {
        Self {
            capture,
            encoder,
            packetizer,
            display_id,
        }
    }

    pub fn start(&mut self) -> Result<(), StreamError> {
        self.encoder.start()?;
        Ok(())
    }

    pub fn encoder_settings(&self) -> &crate::encoder::EncoderSettings {
        self.encoder.settings()
    }

    pub fn encoder_backend_name(&self) -> &str {
        self.encoder.backend_name()
    }

    pub fn capture_encode_packetize(&mut self) -> Result<Vec<TransportPacket>, StreamError> {
        let frame = self.capture.capture_frame(self.display_id)?;
        let encoded = self.encoder.encode(&frame)?;
        packetize_encoded_frame(&self.packetizer, encoded)
    }
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
        packetize_encoded_frame(&self.packetizer, encoded)
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

impl<C> AudioPacketPipeline<C>
where
    C: AudioCapture,
{
    pub fn new(capture: C) -> Self {
        let config = capture.config();
        Self {
            capture,
            packetizer: AudioPacketizer::new(config),
            next_transport_sequence: 1,
        }
    }

    pub fn capture_packetize(&mut self) -> Result<TransportPacket, StreamError> {
        let frame = self.capture.capture_audio_frame()?;
        let config = self.capture.config();
        validate_audio_frame(&frame, config)?;
        let audio = self
            .packetizer
            .packetize(frame.capture_timestamp_us, frame.pcm16)?;
        let payload = encode_frame(audio.sequence, &Message::AudioFrame(audio))?;
        let sequence = self.next_transport_sequence;
        self.next_transport_sequence = self.next_transport_sequence.saturating_add(1);
        Ok(TransportPacket {
            sequence,
            channel: ChannelKind::Audio,
            message_kind: MessageKind::AudioFrame,
            enqueue_timestamp_us: frame.capture_timestamp_us,
            payload,
        })
    }
}

fn packetize_encoded_frame(
    packetizer: &VideoPacketizer,
    encoded: EncodedVideoFrame,
) -> Result<Vec<TransportPacket>, StreamError> {
    let access_unit = EncodedVideoAccessUnit {
        sequence: encoded.sequence,
        codec: encoded.codec,
        presentation_time_us: encoded.capture_timestamp_us,
        is_keyframe: encoded.is_keyframe,
        payload: encoded.payload,
    };

    Ok(packetizer.packetize(&access_unit)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::FakeAudioCapture;
    use glyphray_audio::{AudioCodec, AudioConfig};
    use glyphray_protocol::{decode_frame, AudioFrame};

    #[test]
    fn audio_packet_pipeline_emits_audio_channel_protocol_frame() {
        let config = AudioConfig {
            codec: AudioCodec::Pcm16,
            sample_rate: 48_000,
            channels: 2,
            frame_duration_ms: 10,
        };
        let mut pipeline = AudioPacketPipeline::new(FakeAudioCapture::new(
            config,
            [vec![0x10, 0x00, 0x20, 0x00]],
        ));

        let packet = pipeline.capture_packetize().expect("packet");
        assert_eq!(packet.sequence, 1);
        assert_eq!(packet.channel, ChannelKind::Audio);
        assert_eq!(packet.message_kind, MessageKind::AudioFrame);

        let frame = decode_frame(&packet.payload).expect("protocol frame");
        let Message::AudioFrame(AudioFrame {
            sequence,
            sample_rate,
            channels,
            payload,
            ..
        }) = frame.message
        else {
            panic!("expected AudioFrame")
        };
        assert_eq!(sequence, 1);
        assert_eq!(sample_rate, 48_000);
        assert_eq!(channels, 2);
        assert_eq!(payload, vec![0x10, 0x00, 0x20, 0x00]);
    }

    #[test]
    fn audio_packet_pipeline_rejects_empty_payloads() {
        let config = AudioConfig {
            codec: AudioCodec::Pcm16,
            ..AudioConfig::default()
        };
        let mut pipeline = AudioPacketPipeline::new(FakeAudioCapture::new(config, [Vec::new()]));

        assert!(matches!(
            pipeline.capture_packetize(),
            Err(StreamError::AudioCapture(AudioCaptureError::EmptyPayload))
        ));
    }
}
