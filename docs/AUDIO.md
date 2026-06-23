# Audio

Audio is a Milestone 5 feature. The repository now includes `crates/audio` with:

- audio configuration
- PCM/Opus codec intent
- `AudioPacketizer`
- protocol `AudioFrame` integration
- macOS host audio permission status and request UI
- Android `AudioFrame` decode and low-latency PCM16 `AudioTrack` playback foundation
- Windows host AudioFrame packet pipeline and secure approved-peer Audio-channel queueing

## Android Playback

The Android client now accepts encrypted transport packets on the Audio channel when the
message kind is `AudioFrame`. The payload is decoded through the same `GLYR` frame and
bincode layout used by the Rust protocol enum, then written to an `AudioTrack` in
`MODE_STREAM` with `PERFORMANCE_MODE_LOW_LATENCY`.

The current playback path intentionally accepts PCM16 mono/stereo only. It is a real
runtime boundary, but still a foundation: it does not yet implement Opus decode,
audio/video clock drift correction, jitter buffering, or underrun recovery telemetry.

## Windows Host Packet Path

The Windows host now has a testable audio capture boundary and an `AudioPacketPipeline`.
Captured PCM16 frames are validated against the configured sample rate and channel count,
wrapped with `crates/audio::AudioPacketizer`, encoded as a `GLYR` `AudioFrame`, and emitted
as a `GLYT` Audio-channel `TransportPacket`.

`HostBackendRuntime::queue_audio_packets_for_approved_peers` mirrors the video queueing path:
only approved peers with an established secure session receive audio packets, and audio queue
depth / metrics are visible in the backend health snapshot.

The concrete `WindowsWasapiAudioCapture` type exists as the runtime boundary. It currently
returns an explicit unavailable error until WASAPI loopback capture is wired into the worker.

Next implementation work:

- Windows WASAPI loopback capture worker.
- macOS AVFoundation/CoreAudio capture.
- Android device playback validation with long-running host streams.
- Opus encode/decode integration.
- Drift correction between video and audio clocks.
