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
- Lightweight UDP control runtime on the GlyphRay control port. It accepts Android manual-host `PairingRequest` messages, returns `PairingResult`, records the approved client endpoint, replies to latency pings, and records client video preferences.
- Keychain-backed trusted-client persistence for approved macOS clients. The host restores saved clients on launch and includes a UI clear action for local smoke testing.
- Signed returning-client authentication. The first accepted Android client stores its Keystore public-key DER and SHA-256 trusted id; later matching pairing requests receive `AuthChallenge`, and the host verifies the Android ECDSA `AuthResponse` before sending an accepted `PairingResult`.
- LAN discovery advertiser that sends `GLYD` host advertisements for Android host-list visibility on local networks.
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

Approved-client live streaming is partially wired. The control runtime can learn the Android client endpoint from a pairing request, persists approved client records in Keychain, and the UI automatically copies the newest endpoint into the video target fields. Starting Control also starts a `GLYD` discovery advertiser so Android can discover the macOS host on networks that allow broadcast. The UDP send probe and continuous stream path verify that an `SCStream` can produce frames, those sample buffers can be passed into `VideoToolboxEncoder`, encoded access units can be packetized into GlyphRay Video-channel datagrams, and those datagrams can be sent over UDP. The next macOS-specific step is to validate the signed trusted-device path on macOS CI and real Android hardware, then harden it with encrypted transport, reconnect, and explicit backpressure behavior.

Manual loopback flow:

1. Start the macOS host and press `Start Control`.
2. On Android, use the discovered macOS host, or add the macOS host IP manually with UDP port `44999` if broadcast is blocked.
3. Connect and send pairing from Android.
4. Confirm macOS lists the approved client and copies its endpoint into the UDP target fields.
5. Press `Start UDP Stream` after Screen Recording permission is granted.

The current trusted-client persistence stores local endpoint, public-key fingerprint, and public-key DER metadata. Returning clients now have a signed challenge/response path, but it still needs macOS CI, real-device replay/expiry testing, and encrypted session transport before production trust semantics.
