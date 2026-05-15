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
- GlyphRay video transport packetizer probe that wraps encoded H.264 frames into `GLYF` fragments and `GLYT` Video-channel datagrams.
- Screen Recording and Accessibility permission checks/prompts.
- Input Monitoring status note for manual review.
- Audio permission status plumbing.
- VideoToolbox H.264 low-latency encoder smoke test.
- CGEvent mouse, click, and keyboard posting foundation.
- Keychain-backed secret store boundary with UI smoke test.

Live UDP streaming is still pending. The live transport probe now verifies that an `SCStream` can produce frames, those sample buffers can be passed into `VideoToolboxEncoder`, and encoded access units can be packetized into GlyphRay Video-channel datagrams. The next macOS-specific step is to add the pairing/control runtime and send those datagrams to approved clients.
