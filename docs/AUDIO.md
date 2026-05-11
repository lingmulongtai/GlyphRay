# Audio

Audio is a Milestone 5 feature. The repository now includes `crates/audio` with:

- audio configuration
- PCM/Opus codec intent
- `AudioPacketizer`
- protocol `AudioFrame` integration

Next implementation work:

- Windows WASAPI capture.
- macOS AVFoundation/CoreAudio capture.
- Android AudioTrack playback.
- Opus encode/decode integration.
- Drift correction between video and audio clocks.

