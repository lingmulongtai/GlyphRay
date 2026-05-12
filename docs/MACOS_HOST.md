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
- Screen Recording and Accessibility permission checks/prompts.
- Input Monitoring status note for manual review.
- Audio permission status plumbing.
- VideoToolbox H.264 low-latency encoder smoke test.
- CGEvent mouse, click, and keyboard posting foundation.
- Keychain-backed secret store boundary with UI smoke test.

Live capture-to-encode-to-transport streaming is still pending. The next macOS-specific step is to turn the ScreenCaptureKit display listing into an `SCStream`, feed frames into `VideoToolboxEncoder`, and send encoded access units through the shared transport.
