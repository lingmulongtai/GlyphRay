# Roadmap

Current overall progress estimate: 66%.

## Milestone 1

- [x] Create monorepo structure.
- [x] Add Rust workspace crates.
- [x] Add binary protocol frame and stylus schema.
- [x] Add coordinate mapping tests.
- [x] Add pressure curve tests.
- [x] Add security/session helper tests.
- [x] Add transport simulation tests.
- [x] Add Android Compose skeleton.
- [x] Fix Compose layout dependency and stylus diagnostics pointer-input compile path.
- [x] Add Android stylus diagnostics screen.
- [x] Add Windows host skeleton.
- [x] Add Windows pen injection wrapper.
- [x] Add Windows pen diagnostic CLI.
- [x] Add macOS host shell.
- [x] Add initial docs and CI.
- [x] Add visual README and Japanese README for repository onboarding.

## Milestone 2

- [x] Implement Windows monitor enumeration.
- [ ] Implement Windows Graphics Capture or Desktop Duplication capture path.
- [x] Add encoder trait with H.264 low-latency configuration.
- [ ] Add at least one hardware encoder path or a clean software fallback.
- [x] Add Android MediaCodec H.264 decode pipeline.
- [x] Add Android LAN host discovery receiver for host advertisements.
- [x] Add Android video fragment reassembly and decoder feed controller.
- [x] Add UDP datagram packet format for transport packets.
- [x] Add video frame chunking and reassembly utilities.
- [x] Add Windows capture to encode to packetize streaming pipeline.
- [x] Add Windows GDI capture fallback for early local validation.
- [x] Add Windows capture diagnostic CLI.
- [x] Add server-side UDP socket for host backend.
- [x] Add LAN host discovery advertisement.
- [ ] Stream video over LAN using the transport abstraction.
- [x] Add secure datagram codec foundation for encrypted transport.
- [x] Add latency overlay prepared for `glyphray-telemetry`.

## Milestone 3

- [x] Add Android compact stylus packet encoder.
- [x] Add Android UDP transport packet encoder for compact stylus batches.
- [x] Add Rust compact stylus packet decoder/encoder.
- [x] Add Windows host stylus input bridge.
- [x] Add backend session registry and input packet routing.
- [x] Add backend permission gate for input packets.
- [ ] Connect Android stylus packets to Windows native pen injection over LAN.
- [ ] Validate pressure, tilt, hover, barrel button, and eraser in creative apps.
- [x] Add drawing area calibration UI surface.
- [x] Add calibration profile math.
- [ ] Harden multi-monitor and high-DPI mapping on real hardware.

## Milestone 4

- [x] Add session cipher and secure datagram codec foundation.
- [x] Add Android Keystore device identity foundation.
- [x] Add Windows platform secret-store boundary.
- [x] Add reconnect and adaptive bitrate controllers.
- [x] Add host diagnostics CLI.
- [x] Add console backend runtime entry point.
- [x] Add development-only backend auto-approval mode for LAN input smoke tests.
- [x] Fix Rust CI HMAC initialization ambiguity.
- [x] Add installer packaging foundation.
- [ ] Replace Windows development secret store with DPAPI or Credential Manager.
- [ ] Add macOS Keychain implementation.
- [ ] Add signed production installers.

## Milestone 5

- [x] Add macOS ScreenCaptureKit display enumeration foundation.
- [x] Add macOS VideoToolbox encoder foundation.
- [x] Add macOS CGEvent input foundation.
- [x] Add audio packetization primitives.
- [x] Add optional relay architecture notes and route selection logic.
- [x] Add beta release checklist.
- [ ] Complete macOS host live capture and encode stream.
- [ ] Add Windows/macOS audio capture and Android playback.
- [ ] Implement relay server/client.
- [ ] Prepare signed beta release.
