# macOS Host

macOS support is Phase 2. The repository includes a Swift Package shell in `hosts/macos-host`.

## Planned Native APIs

- UI: SwiftUI
- Screen capture: ScreenCaptureKit
- Video encoding: VideoToolbox
- Input: CGEvent for mouse and keyboard first
- Secrets: Keychain

## Pen Support

Native Windows Ink-style pen injection is Windows-specific. macOS can support pointer, keyboard, and possibly tablet-event-compatible paths later, but the initial macOS host should not claim Windows Ink parity.

## Build

```bash
cd hosts/macos-host
swift build
```

Current code includes:

- SwiftUI shell with local readiness diagnostics.
- ScreenCaptureKit display listing with display geometry labels.
- ScreenCaptureKit live capture probe that opens an `SCStream`, counts screen frames for a short run, then stops.
- ScreenCaptureKit-to-VideoToolbox live encode probe that feeds captured `CMSampleBuffer` images into the low-latency H.264 encoder and counts encoded frames/bytes.
- H.264 Annex B conversion for VideoToolbox output, including SPS/PPS on keyframes for Android decoder readiness.
- GlyphRay video transport packetizer probe that wraps encoded H.264 frames into `GLYF` fragments and `GLYT` Video-channel datagrams.
- Manual UDP send probe that sends generated Video-channel datagrams to a typed host/port for receiver-side smoke tests.
- Continuous UDP video stream start/stop path that keeps an `SCStream` running, packetizes each encoded frame, and publishes Video-channel datagrams to a typed manual target.
- Screen Recording and Accessibility permission checks/prompts.
- Input Monitoring status note for manual review.
- Audio permission status plumbing.
- VideoToolbox H.264 low-latency encoder smoke test.
- CGEvent mouse, click, and keyboard posting foundation.
- Keychain-backed secret store boundary with UI smoke test.

Approved-client live streaming is still pending. The UDP send probe and continuous stream path now verify that an `SCStream` can produce frames, those sample buffers can be passed into `VideoToolboxEncoder`, encoded access units can be packetized into GlyphRay Video-channel datagrams, and those datagrams can be sent over UDP to a manual target. The next macOS-specific step is to add pairing/control runtime state so the sender targets approved clients automatically and owns reconnect/backpressure behavior.
