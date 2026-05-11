# GlyphRay

GlyphRay is a low-latency remote creative desktop app foundation for artists, designers, and illustrators who want to use an Android tablet or phone as a high-quality remote pen display for a Windows or macOS computer.

The product goal is Parsec-like speed and simplicity with an original brand, UI, codebase, protocol, and architecture. The key differentiator is native Windows Ink-style pen injection from Android stylus input, not mouse-only emulation.

## Development Progress

**Overall progress estimate: 31%**

Last updated: 2026-05-11 JST

| Area | Status | Progress |
| --- | --- | ---: |
| Milestone 1 foundation | Complete | 100% |
| Milestone 2 video and transport foundation | In progress | 55% |
| Milestone 3 Android stylus to Windows Ink stream | Not started | 0% |
| Milestone 4 hardening and packaging | Not started | 0% |
| Milestone 5 macOS, audio, relay readiness | Not started | 0% |

Development diary: [docs/DEVELOPMENT_DIARY.md](docs/DEVELOPMENT_DIARY.md)

## What Exists Now

- Rust workspace for shared core, protocol, transport, security, and telemetry.
- Versioned binary protocol with stylus, media, session, pairing, latency, and control messages.
- Coordinate mapping and pressure-curve logic with unit tests.
- Android Jetpack Compose client skeleton with host, pairing, session, pen settings, video settings, and diagnostics screens.
- Android stylus diagnostics reading raw `MotionEvent` pressure, tilt, orientation, hover, buttons, eraser, history, and timestamps.
- Android low-latency `SurfaceView` plus `MediaCodec` H.264 decoder foundation.
- Windows Rust host skeleton with pairing, monitor enumeration, encoder abstraction, and synthetic pen injection wrapper.
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

Milestone 2 now needs real Windows screen capture, a concrete H.264 encoder backend, packetization of encoded frames over LAN, and Android decoder feed plumbing from the transport layer.
