# GlyphRay

GlyphRay is a low-latency remote creative desktop app foundation for artists, designers, and illustrators who want to use an Android tablet or phone as a high-quality remote pen display for a Windows or macOS computer.

The product goal is Parsec-like speed and simplicity with an original brand, UI, codebase, protocol, and architecture. The key differentiator is native Windows Ink-style pen injection from Android stylus input, not mouse-only emulation.

## Development Progress

**Overall progress estimate: 62%**

Last updated: 2026-05-11 JST

| Area | Status | Progress |
| --- | --- | ---: |
| Milestone 1 foundation | Complete | 100% |
| Milestone 2 video and transport foundation | In progress | 83% |
| Milestone 3 Android stylus to Windows Ink stream | In progress | 52% |
| Milestone 4 hardening and packaging | In progress | 44% |
| Milestone 5 macOS, audio, relay readiness | In progress | 35% |

Development diary: [docs/DEVELOPMENT_DIARY.md](docs/DEVELOPMENT_DIARY.md)

## What Exists Now

- Rust workspace for shared core, protocol, transport, security, and telemetry.
- Versioned binary protocol with stylus, media, session, pairing, latency, and control messages.
- Coordinate mapping and pressure-curve logic with unit tests.
- Android Jetpack Compose client skeleton with host, pairing, session, pen settings, video settings, and diagnostics screens.
- Android stylus diagnostics reading raw `MotionEvent` pressure, tilt, orientation, hover, buttons, eraser, history, and timestamps.
- Android low-latency `SurfaceView` plus `MediaCodec` H.264 decoder foundation.
- Android video fragment reassembly and decoder feed controller.
- Android compact stylus packet encoder and calibration UI surface.
- Android Keystore device identity key foundation.
- Windows Rust host skeleton with pairing, monitor enumeration, GDI capture fallback, encoder abstraction, streaming pipeline, and synthetic pen injection wrapper.
- Windows backend runtime with LAN discovery, UDP server routing, session registry, pairing request handling, permission gating, and latency pong replies.
- Windows stylus input bridge for forwarding remote batches to native pen injection.
- Session cipher and secure datagram codec using ChaCha20-Poly1305.
- Reconnect backoff and adaptive bitrate controllers.
- Audio packetization primitives.
- Relay candidate selection logic.
- macOS VideoToolbox, CGEvent, and audio-capture controller foundations.
- Windows pen diagnostic CLI for synthetic pressure-stroke injection.
- UDP datagram transport packet format with checksum validation.
- Video frame chunking/reassembly utilities for UDP-sized packets.
- macOS SwiftUI host shell prepared for ScreenCaptureKit and VideoToolbox work.
- Product, architecture, protocol, security, performance, roadmap, and test documentation.
- GitHub Actions CI for Rust tests and Android debug build.

## Repository Layout

```text
apps/android-client       Android Kotlin + Jetpack Compose client
hosts/windows-host        Rust Windows host and diagnostic tools
hosts/macos-host          SwiftUI macOS host shell
crates/core               Coordinate mapping, pressure curves, session models
crates/protocol           Versioned binary wire protocol
crates/transport          Low-latency transport abstractions and UDP packet layer
crates/security           Pairing/session primitives and secret-store interfaces
crates/telemetry          Local latency/performance metrics primitives
docs                      Product, architecture, security, roadmap, diary
tools                     Cross-platform development tools
tests                     Integration test home
```

## Build

### Rust Workspace

Install stable Rust:

```powershell
cargo test --workspace
```

Run the Windows pen diagnostic tool on Windows 10/11:

```powershell
cargo run -p glyphray-pen-diagnostics
```

Run the Windows capture diagnostic tool:

```powershell
cargo run -p glyphray-capture-diagnostics
```

Run the Windows host diagnostics tool:

```powershell
cargo run -p glyphray-host-diagnostics
```

Run the host backend runtime:

```powershell
cargo run -p glyphray-windows-host -- serve
```

### Android Client

Install Android Studio or Android SDK command-line tools:

```powershell
gradle :apps:android-client:assembleDebug
```

The Android app currently includes stylus diagnostics, a session UI, a latency overlay, and a MediaCodec-backed decoder surface prepared for incoming H.264 frames.

### macOS Host

On macOS 13+ with Xcode installed:

```bash
cd hosts/macos-host
swift build
```

The macOS host is still Phase 2/5 scaffolding. Windows remains the primary platform for pen injection.

## Verification Notes

This workspace was generated on a machine where `cargo`, `gradle`, `swift`, and Android SDK tools were not installed on `PATH`, so local full builds could not be executed here. XML parsing and repository structure checks were run locally; CI is configured to perform the real Rust and Android checks.

## Next Focus

The next hardest work is replacing fallback capture with Windows Graphics Capture or Desktop Duplication, adding a concrete H.264 encoder backend, wiring backend peer approval to UI, driving the video pipeline continuously, and validating Windows Ink pressure/tilt/hover in real creative apps.
