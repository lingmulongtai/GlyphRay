use glyphray_transport::udp::{decode_packet, encode_packet};
use glyphray_transport::video::{EncodedVideoAccessUnit, VideoPacketizer, VideoReassembler};
use glyphray_windows_host::capture::{CapturedFrame, ScreenCapture, WindowsGraphicsCaptureBackend};
use glyphray_windows_host::encoder::{
    available_h264_hardware_encoders, parse_encoder_backend, EncoderError, EncoderSettings,
    PlatformVideoEncoder, VideoEncoder,
};
use std::time::Instant;

fn main() {
    if let Err(error) = run() {
        eprintln!("Encoder diagnostic failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    match available_h264_hardware_encoders() {
        Ok(encoders) if encoders.is_empty() => {
            println!("Hardware H.264 MFT candidates: none");
        }
        Ok(encoders) => {
            println!("Hardware H.264 MFT candidates:");
            for encoder in encoders {
                println!("  {:?}: {}", encoder.backend, encoder.friendly_name);
            }
        }
        Err(error) => eprintln!("Hardware H.264 MFT enumeration failed: {error}"),
    }

    let mut capture = WindowsGraphicsCaptureBackend::new();
    let displays = capture.list_displays()?;
    let display = displays
        .iter()
        .find(|display| display.primary)
        .or_else(|| displays.first())
        .ok_or("no display is available")?;

    let first_frame = match capture.capture_frame(display.id) {
        Ok(frame) => frame,
        Err(error) => {
            eprintln!(
                "Capture unavailable ({error}); using a synthetic BGRA frame to isolate the encoder."
            );
            synthetic_frame(display.id, 1280, 720)
        }
    };

    let mut settings = EncoderSettings::low_latency_h264(first_frame.width, first_frame.height, 60);
    if let Ok(value) = std::env::var("GLYPHRAY_ENCODER_BACKEND") {
        settings.backend = parse_encoder_backend(&value).ok_or(
            "GLYPHRAY_ENCODER_BACKEND must be auto, hardware, intel, nvidia, amd, or software",
        )?;
    }
    let mut encoder = PlatformVideoEncoder::new(settings.clone());
    encoder.start()?;

    println!(
        "Encoding display {} at {}x{}, {}fps, {}kbps with {:?} ({})",
        display.id,
        settings.width,
        settings.height,
        settings.fps,
        settings.target_bitrate_kbps,
        encoder.settings().backend,
        encoder.backend_name()
    );

    for attempt in 1..=120 {
        let frame = if attempt == 1 {
            first_frame.clone()
        } else {
            capture
                .capture_frame(display.id)
                .unwrap_or_else(|_| first_frame.clone())
        };
        let started = Instant::now();
        match encoder.encode(&frame) {
            Ok(encoded) => {
                let elapsed = started.elapsed();
                if encoded.payload.is_empty() {
                    return Err("encoder returned an empty H.264 access unit".into());
                }
                let annex_b = encoded.payload.starts_with(&[0, 0, 1])
                    || encoded.payload.starts_with(&[0, 0, 0, 1]);
                println!(
                    "Encoded sequence={} bytes={} keyframe={} annex_b={} crc32={:08x} encode_ms={:.3}",
                    encoded.sequence,
                    encoded.payload.len(),
                    encoded.is_keyframe,
                    annex_b,
                    crc32fast::hash(&encoded.payload),
                    elapsed.as_secs_f64() * 1_000.0
                );
                if !annex_b {
                    return Err("encoder output is not Annex B H.264".into());
                }
                verify_video_datagrams(&encoded)?;
                return Ok(());
            }
            Err(EncoderError::OutputUnavailable) if attempt < 120 => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Err("encoder did not produce output after 120 submitted frames".into())
}

fn verify_video_datagrams(
    encoded: &glyphray_windows_host::encoder::EncodedVideoFrame,
) -> Result<(), Box<dyn std::error::Error>> {
    let access_unit = EncodedVideoAccessUnit {
        sequence: encoded.sequence,
        codec: encoded.codec,
        presentation_time_us: encoded.capture_timestamp_us,
        is_keyframe: encoded.is_keyframe,
        payload: encoded.payload.clone(),
    };
    let packets = VideoPacketizer::default().packetize(&access_unit)?;
    let mut reassembler = VideoReassembler::default();
    let mut completed = None;
    let mut wire_bytes = 0_usize;
    let mut largest_datagram = 0_usize;
    for packet in &packets {
        let datagram = encode_packet(packet)?;
        wire_bytes += datagram.len();
        largest_datagram = largest_datagram.max(datagram.len());
        let decoded = decode_packet(&datagram)?;
        if let Some(frame) = reassembler.push_packet(&decoded)? {
            completed = Some(frame);
        }
    }

    if completed.as_ref() != Some(&access_unit) {
        return Err("video fragments did not reconstruct the original H.264 access unit".into());
    }
    println!(
        "Packetized and reassembled {} UDP datagrams (wire_bytes={}, max_datagram={}, payload_crc32={:08x})",
        packets.len(),
        wire_bytes,
        largest_datagram,
        crc32fast::hash(&access_unit.payload)
    );
    Ok(())
}

fn synthetic_frame(display_id: u32, width: u32, height: u32) -> CapturedFrame {
    let mut bgra = vec![0_u8; width as usize * height as usize * 4];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let offset = (y * width as usize + x) * 4;
            bgra[offset] = ((x * 255) / width.max(1) as usize) as u8;
            bgra[offset + 1] = ((y * 255) / height.max(1) as usize) as u8;
            bgra[offset + 2] = 96;
            bgra[offset + 3] = 255;
        }
    }
    CapturedFrame {
        display_id,
        width,
        height,
        capture_timestamp_us: 0,
        bgra,
    }
}
