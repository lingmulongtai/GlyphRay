# Roadmap

Current overall progress estimate: 98%.

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
- [x] Verify Android debug build with Gradle wrapper.
- [x] Add Android stylus diagnostics screen.
- [x] Add Windows host skeleton.
- [x] Add Windows pen injection wrapper.
- [x] Add Windows pen diagnostic CLI.
- [x] Add macOS host shell.
- [x] Add initial docs and CI.
- [x] Add GitHub Actions macOS SwiftPM build verification for the macOS host.
- [x] Add visual README and Japanese README for repository onboarding.
- [x] Add frontend-only GitHub Pages download site.

## Milestone 2

- [x] Implement Windows monitor enumeration.
- [ ] Implement Windows Graphics Capture or Desktop Duplication capture path.
- [x] Add encoder trait with H.264 low-latency configuration.
- [ ] Add at least one hardware encoder path or a clean software fallback.
- [x] Add Android MediaCodec H.264 decode pipeline.
- [x] Add Android LAN host discovery receiver for host advertisements.
- [x] Add Android control-channel sender for pairing and latency smoke tests.
- [x] Add Android control-channel receiver for pairing and latency responses.
- [x] Add Android display-info decode for host monitor negotiation.
- [x] Add Android host display selector and attach selected display id to stylus/touch/mouse packets.
- [x] Add Android video/session setting controls for resolution, refresh, bitrate, color space, codec, fullscreen, touch mode, keyboard, and special keys.
- [x] Persist Android video/input session preferences across app restarts.
- [x] Polish Android host list, connection readiness, session cockpit, settings, security, and diagnostics UI/UX.
- [x] Add and persist Android manual host entries for Tailscale/overlay-network endpoints.
- [x] Add Android video fragment reassembly and decoder feed controller.
- [x] Add UDP datagram packet format for transport packets.
- [x] Add video frame chunking and reassembly utilities.
- [x] Add Windows capture to encode to packetize streaming pipeline.
- [x] Add Windows GDI capture fallback for early local validation.
- [x] Add Windows capture diagnostic CLI.
- [x] Add server-side UDP socket for host backend.
- [x] Add LAN host discovery advertisement.
- [x] Queue H.264 access-unit fragments on the Video channel for approved clients.
- [x] Route Android Video channel packets into the video reassembler and MediaCodec feed path.
- [ ] Stream real encoded desktop video over LAN using a concrete H.264 backend.
- [x] Add secure datagram codec foundation for encrypted transport.
- [x] Add latency overlay prepared for `glyphray-telemetry`.

## Milestone 3

- [x] Add Android compact stylus packet encoder.
- [x] Add Android UDP transport packet encoder for compact stylus batches.
- [x] Wire remote-session stylus capture to Android LAN UDP sender.
- [x] Add Rust compact stylus packet decoder/encoder.
- [x] Add Android protocol-frame unit coverage and wire it into CI.
- [x] Add Windows host stylus input bridge.
- [x] Add stylus pressure smoothing and pen-axis normalization before Win32 injection.
- [x] Add opt-in native pen injection bridge to Windows backend serve runtime.
- [x] Add backend session registry and input packet routing.
- [x] Add backend permission gate for input packets.
- [x] Add console approval/rejection flow with `PairingResult` responses.
- [x] Add opt-in native Windows permission dialog for incoming pairing requests.
- [x] Persist approved Windows host trusted-device records and add host console trust management commands.
- [x] Attach Android Keystore public key material to pairing and require signed `AuthChallenge` / `AuthResponse` proof before approving returning trusted devices.
- [x] Queue host `DisplayInfo` after accepted pairing.
- [x] Accept client `EncoderConfig` and decode keyboard packets on the host backend.
- [x] Implement host-side encoder override CLI and connect approved-client settings to the live capture/encode loop.
- [x] Persist saved Windows host encoder override presets and reload them on backend startup.
- [x] Add named Windows host encoder presets for quick stream-quality switching during hardware validation.
- [x] Add native Windows keyboard injection for approved clients behind an explicit smoke-test flag.
- [x] Add native Windows mouse injection for approved clients behind an explicit smoke-test flag.
- [x] Add Android direct-touch protocol path and Windows native touch injection smoke-test path.
- [x] Add Android gamepad protocol path and Windows host gamepad decode.
- [ ] Add virtual gamepad injection backend for Windows.
- [x] Add first-pass touch gesture translation for trackpad and gesture assist modes.
- [x] Add Android system-bar fullscreen handling.
- [ ] Connect Android stylus packets to Windows native pen injection over LAN.
- [ ] Validate pressure, tilt, hover, barrel button, and eraser in creative apps.
- [x] Add drawing area calibration UI surface.
- [x] Add calibration profile math.
- [x] Use selected display geometry for runtime input mapper when Windows display enumeration is available.
- [ ] Harden multi-monitor and high-DPI mapping on real hardware.

## Milestone 4

- [x] Add session cipher and secure datagram codec foundation.
- [x] Add Android Keystore device identity foundation.
- [x] Add Windows platform secret-store boundary.
- [x] Add reconnect and adaptive bitrate controllers.
- [x] Add pending-session cap and late input packet dropping in the host router.
- [x] Add per-IP pending attempt rate limiting for UDP source-port churn.
- [x] Add bounded nonblocking QoS send queues for the host polling loop.
- [x] Expose backend health snapshots and console status output for queue/backpressure/late-drop counters.
- [x] Replace ad hoc host discovery ID hashing with CRC-based stable hashing.
- [x] Add host diagnostics CLI.
- [x] Add console backend runtime entry point.
- [x] Add development-only backend auto-approval mode for LAN input smoke tests.
- [x] Fix Rust CI HMAC initialization ambiguity.
- [x] Add installer packaging foundation.
- [x] Add Windows WiX MSI build script and macOS pkgbuild script.
- [x] Add Windows user-logon startup CLI and service/agent architecture plan.
- [ ] Move control/video packet transmission onto a dedicated send worker or mio/tokio event loop with explicit backpressure metrics.
- [ ] Replace O(N) pending-session eviction with heap/indexed eviction if reused for relay-scale workloads.
- [ ] Validate and document lock-screen/pre-login limitations for capture and input.
- [x] Add GitHub Pages deployment workflow for the download site.
- [x] Replace Windows development secret store with DPAPI-protected per-user storage.
- [x] Add macOS Keychain implementation.
- [ ] Add signed production installers.
- [ ] Add Android Play Store internal testing release track checklist and signing workflow.

## Milestone 5

- [x] Add macOS ScreenCaptureKit display enumeration foundation.
- [x] Add macOS VideoToolbox encoder foundation.
- [x] Add macOS CGEvent input foundation.
- [x] Add macOS permission/readiness diagnostics for Screen Recording, Accessibility, Input Monitoring, and audio.
- [x] Add macOS VideoToolbox low-latency encoder smoke-test path.
- [x] Add macOS ScreenCaptureKit live frame probe.
- [x] Add macOS ScreenCaptureKit-to-VideoToolbox live encode probe.
- [x] Add macOS encoded-frame packetizer for GlyphRay Video-channel datagrams.
- [x] Add macOS manual UDP send probe for generated Video-channel datagrams.
- [x] Add macOS continuous UDP video stream start/stop path for manual receiver loopback.
- [x] Add macOS lightweight UDP control runtime for Android manual-host `PairingRequest`, `PairingResult`, latency pong, and client video preference intake.
- [x] Add macOS LAN discovery advertiser for Android host-list visibility on local networks.
- [x] Persist macOS approved client records in Keychain and restore them on host startup.
- [x] Add macOS signed returning-client `AuthChallenge` / `AuthResponse` verification using Android Keystore public-key DER and SHA-256 trusted ids.
- [x] Add audio packetization primitives.
- [x] Add optional relay architecture notes and route selection logic.
- [x] Add beta release checklist.
- [ ] Harden the macOS lightweight control/discovery runtime with macOS CI/real-device validation, encrypted transport, reconnect, and backpressure-aware stream ownership.
- [ ] Add Windows/macOS audio capture and Android playback.
- [ ] Implement relay server/client.
- [ ] Prepare signed beta release.
