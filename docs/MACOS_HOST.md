# macOS Host

macOS support is the secondary host platform. `hosts/macos-host` is a SwiftPM SwiftUI application targeting macOS 13+.

## Native APIs

- UI: SwiftUI
- Screen capture: ScreenCaptureKit
- Video encoding: VideoToolbox
- Input: CGEvent for mouse, keyboard, and single-touch pointer compatibility
- Secrets: Keychain

## Pen Support

Native Windows Ink-style pen injection is Windows-specific. macOS can support pointer, keyboard, and possibly tablet-event-compatible paths later, but the initial macOS host should not claim Windows Ink parity.

## Build

```bash
cd hosts/macos-host
swift build
swift test -c release
```

Current code includes:

- SwiftUI shell with local readiness diagnostics.
- UDP control runtime on the GlyphRay control port with pairing, signed returning-device authentication, and LAN discovery.
- Keychain-backed trusted-client persistence for approved macOS clients. The host restores saved clients on launch and includes a UI clear action for local smoke testing.
- Salted first-time pairing. New Android clients receive a peer-bound `PairingChallenge` and must prove the six-digit code shown in SwiftUI before trust is stored. The host rotates the code after success and rate limits failures.
- Signed returning-client authentication. The first accepted Android client stores its Keystore public-key DER and SHA-256 trusted id; later matching pairing requests receive `AuthChallenge`, and the host verifies the Android ECDSA `AuthResponse` before sending an accepted `PairingResult`.
- LAN discovery advertiser that sends `GLYD` host advertisements for Android host-list visibility on local networks.
- ScreenCaptureKit display listing with display geometry labels.
- ScreenCaptureKit live capture probe that opens an `SCStream`, counts screen frames for a short run, then stops.
- ScreenCaptureKit-to-VideoToolbox live encode probe that feeds captured `CMSampleBuffer` images into the low-latency H.264 encoder and counts encoded frames/bytes.
- H.264 Annex B conversion for VideoToolbox output, including SPS/PPS on keyframes for Android decoder readiness.
- GlyphRay video transport packetizer probe that wraps encoded H.264 frames into `GLYF` fragments and `GLYT` Video-channel datagrams.
- Keychain-persisted P-256 host identity, signed ephemeral ECDH `GLYH` exchange, directional AES-256-GCM `GLYE` keys, and replay rejection.
- Approved-client UDP stream action that is unavailable until the encrypted session is established.
- Preferred secure-target selection. The UI starts video only for an endpoint with an active encrypted session, favoring the most recently confirmed client and falling back to another secure trusted client.
- Continuous encrypted UDP video stream that keeps an `SCStream` running and packetizes each VideoToolbox access unit.
- Stream ownership IDs and replacement reconnect behavior. Starting a stream for a different encrypted client stops the old stream before opening the new one, so stale video publishers cannot keep sending to an old endpoint.
- Encrypted `DisplayInfo` response and client-selected display, resolution, FPS, bitrate, and keyframe interval.
- Encrypted Android mouse and Bluetooth keyboard CGEvent injection, including button/wheel/modifier mapping.
- Single-finger Android touch translated to macOS pointer down/drag/up compatibility events.
- Bounded UDP video publisher backpressure. The publisher caps in-flight datagrams, records scheduled/sent/dropped counts, bytes, in-flight count, and high watermark so low-latency streams drop stale video instead of accumulating unbounded delay.
- Screen Recording and Accessibility permission checks/prompts.
- Audio permission request button and status plumbing.
- Input Monitoring status note for manual review.
- Audio permission status plumbing.
- VideoToolbox H.264 low-latency encoder smoke test.
- CGEvent mouse, click, wheel, keyboard, and touch-pointer routing from the live Input channel.
- Keychain-backed secret store boundary with UI smoke test.

Approved-client streaming is now cryptographically tied to the paired endpoint. The host sends a signed ephemeral key offer after pairing/authentication, installs the codec only after Android's signed confirmation, sends encrypted display metadata, and exposes streaming only for clients with an active secure session. Every video datagram passes through the secure-session transform before UDP publication. Stream status includes a stream id, reconnect count, high-watermark, and backpressure flag. The next macOS-specific step is GitHub Actions and real Android validation, followed by longer reconnect soak tests and adaptive quality feedback.

Manual loopback flow:

1. Start the macOS host and press `Start Control`.
2. On Android, use the discovered macOS host, or add the macOS host IP manually with UDP port `44999` if broadcast is blocked.
3. Connect and send pairing from Android.
4. Confirm macOS reports one secure client and receives the Android video preference.
5. Press `Start Approved Stream` after Screen Recording permission is granted.

The trusted-client store retains endpoint, public-key fingerprint, and public-key DER metadata in Keychain. The host identity private key is also Keychain-backed. Production trust semantics still require macOS CI confirmation for the newest Swift changes, real-device replay/expiry tests, physical Android streaming, and long-run reconnect/backpressure soak tests.
