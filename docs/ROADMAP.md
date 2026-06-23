# Roadmap

Current implementation progress estimate: 95%. Production release readiness: 82%.

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
- [x] Implement a stateful DXGI Desktop Duplication capture path with D3D11 staging readback, rotation, timeout reuse, and access-loss recreation.
- [x] Add encoder trait with H.264 low-latency configuration.
- [x] Add a Windows Media Foundation H.264 software fallback with low-latency/keyframe controls.
- [x] Add hardware MFT enumeration, Intel/NVIDIA/AMD selection, asynchronous MFT event handling, Auto software fallback, and local NVENC path validation.
- [ ] Validate Intel Quick Sync and AMD AMF encode paths on representative driver/hardware combinations.
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
- [x] Remove the GDI capture fallback after replacing it with Desktop Duplication.
- [x] Report active Windows display refresh rate, DPI scale, rotation, and geometry through display enumeration.
- [x] Add Windows capture diagnostic CLI.
- [x] Add server-side UDP socket for host backend.
- [x] Add LAN host discovery advertisement.
- [x] Queue H.264 access-unit fragments on the Video channel for approved clients.
- [x] Route Android Video channel packets into the video reassembler and MediaCodec feed path.
- [x] Connect concrete Media Foundation H.264 access units to the approved-peer video packet queue.
- [x] Add a runtime diagnostic that encodes Annex B H.264, fragments it into UDP-sized Video packets, round-trips the GLYT datagrams, and reassembles the original access unit.
- [ ] Validate continuous 1080p60 streaming and decoder compatibility on a real Android device.
- [x] Add secure datagram codec foundation for encrypted transport.
- [x] Run control, video, and all realtime Android input over one authenticated encrypted UDP session.
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
- [x] Add Windows virtual gamepad injection bridge, permission-gated router wiring, and normalized XInput-style report boundary.
- [ ] Link and validate a production virtual controller backend such as ViGEm or a signed virtual HID driver.
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
- [x] Add signed P-256 ECDH session negotiation and directional AES-256-GCM transport for Windows/Android.
- [x] Persist the Windows host identity with DPAPI and pin trusted host fingerprints on Android.
- [x] Reject plaintext traffic after key establishment and require secure sessions before video queueing or realtime input.
- [x] Enforce persisted per-device pen/touch/keyboard/mouse/gamepad permissions in the host router.
- [x] Add Android Keystore device identity foundation.
- [x] Require a salted six-digit one-time code before first trust on Windows/macOS, with HMAC proof, expiry, one-use rotation, and attempt throttling.
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
- [x] Add canonical product versioning with Cargo mirror validation and a cross-platform release-candidate workflow.
- [x] Build Android release APK/AAB, Windows MSI, macOS app/pkg, and SHA-256 manifests in CI.
- [x] Block tagged releases unless Android, Windows, and macOS signing plus macOS notarization credentials are present.
- [x] Add a real macOS `.app` bundle layout with privacy usage descriptions and ad-hoc smoke-test signing.
- [x] Add Windows user-logon startup CLI and service/agent architecture plan.
- [ ] Move control/video packet transmission onto a dedicated send worker or mio/tokio event loop with explicit backpressure metrics.
- [ ] Replace O(N) pending-session eviction with heap/indexed eviction if reused for relay-scale workloads.
- [ ] Validate and document lock-screen/pre-login limitations for capture and input.
- [x] Add GitHub Pages deployment workflow for the download site.
- [x] Replace Windows development secret store with DPAPI-protected per-user storage.
- [x] Add macOS Keychain implementation.
- [x] Atomically replace Windows settings/DPAPI files and quarantine corrupt host state before recovery.
- [x] Make macOS Keychain identity/trust updates gap-free and quarantine corrupt records before recovery.
- [x] Add rotating fixed-schema Windows event logs that exclude raw keyboard and secret material.
- [ ] Add signed production installers.
- [x] Add Android Play Store signing workflow driven only by CI secrets.
- [ ] Upload a signed AAB to the Play Store internal testing track and complete store compliance.

## Milestone 5

- [x] Add macOS ScreenCaptureKit display enumeration foundation.
- [x] Add macOS VideoToolbox encoder foundation.
- [x] Add macOS CGEvent input foundation.
- [x] Add macOS permission/readiness diagnostics for Screen Recording, Accessibility, Input Monitoring, and audio.
- [x] Add macOS VideoToolbox low-latency encoder smoke-test path.
- [x] Add macOS ScreenCaptureKit live frame probe.
- [x] Add macOS ScreenCaptureKit-to-VideoToolbox live encode probe.
- [x] Add macOS encoded-frame packetizer for GlyphRay Video-channel datagrams.
- [x] Retire the macOS manual plaintext UDP sender after the encrypted approved-client path landed.
- [x] Add macOS continuous UDP video stream start/stop path for manual receiver loopback.
- [x] Add macOS lightweight UDP control runtime for Android manual-host `PairingRequest`, `PairingResult`, latency pong, and client video preference intake.
- [x] Add macOS LAN discovery advertiser for Android host-list visibility on local networks.
- [x] Persist macOS approved client records in Keychain and restore them on host startup.
- [x] Add macOS signed returning-client `AuthChallenge` / `AuthResponse` verification using Android Keystore public-key DER and SHA-256 trusted ids.
- [x] Add macOS bounded UDP video publisher backpressure counters for continuous stream probes.
- [x] Add macOS approved-client stream action that targets the newest paired Android endpoint.
- [x] Add macOS audio permission request path in the host UI.
- [x] Add Keychain-backed macOS host signing identity and signed `GLYH` P-256 ECDH key exchange.
- [x] Add directional AES-256-GCM `GLYE` protection and replay rejection for macOS sessions.
- [x] Require encryption for macOS approved-client video, latency, mouse, keyboard, and touch-pointer traffic.
- [x] Send encrypted macOS display metadata after key confirmation.
- [x] Apply client-selected display, resolution, FPS, bitrate, and keyframe interval to macOS capture/encoding.
- [x] Route encrypted Android mouse, keyboard, and single-touch input through macOS CGEvent injection.
- [x] Add macOS secure-session and Android-compatible input wire tests to SwiftPM CI.
- [x] Add audio packetization primitives.
- [x] Add optional relay architecture notes and route selection logic.
- [x] Add beta release checklist.
- [x] Add macOS secure-target stream ownership, encrypted target selection, reconnection replacement, and backpressure visibility.
- [ ] Validate the macOS encrypted runtime on GitHub Actions and physical Android hardware with long-run reconnect/backpressure soak tests.
- [x] Add Android AudioFrame decode and low-latency PCM16 AudioTrack playback foundation.
- [x] Add Windows host AudioFrame packet pipeline and secure approved-peer Audio-channel queueing.
- [ ] Add Windows WASAPI loopback capture, macOS audio capture, Opus encode/decode, and Android clock-drift correction.
- [ ] Implement relay server/client.
- [ ] Prepare signed beta release.
