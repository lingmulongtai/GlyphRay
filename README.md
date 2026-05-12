# GlyphRay

[日本語版 README](README.ja.md)

GlyphRay is a low-latency remote creative desktop application for artists, designers, and illustrators who want to use an Android tablet or phone as a high-quality remote pen display for a Windows or macOS computer.

The product goal is Parsec-like speed and simplicity with an original brand, UI, codebase, protocol, and architecture. The key differentiator is native Windows Ink-style pen injection from Android stylus input, not mouse-only emulation.

## Current Progress

**Overall progress estimate: 69%**

Last updated: 2026-05-11 JST

```mermaid
pie title Overall Completion
  "Implemented foundation" : 69
  "Remaining product work" : 31
```

| Area | Status | Progress |
| --- | --- | ---: |
| Milestone 1 foundation | Complete | 100% |
| Milestone 2 video and transport foundation | In progress | 86% |
| Milestone 3 Android stylus to Windows Ink stream | In progress | 63% |
| Milestone 4 hardening and packaging | In progress | 46% |
| Milestone 5 macOS, audio, relay readiness | In progress | 35% |

```text
M1 Foundation                 [####################] 100%
M2 Video + Transport          [#################---]  86%
M3 Stylus -> Windows Ink      [#############-------]  63%
M4 Security + Packaging       [#########-----------]  46%
M5 macOS + Audio + Relay      [#######-------------]  35%
```

Development diary: [docs/DEVELOPMENT_DIARY.md](docs/DEVELOPMENT_DIARY.md)

## What This Repository Contains

```mermaid
flowchart TB
  Root["GlyphRay Monorepo"]
  Android["apps/android-client\nKotlin, Jetpack Compose, MotionEvent, MediaCodec"]
  Windows["hosts/windows-host\nRust, Win32 capture/input, backend runtime"]
  Mac["hosts/macos-host\nSwiftUI, ScreenCaptureKit, VideoToolbox"]
  Crates["crates/*\nRust shared protocol, transport, security, core"]
  Docs["docs/*\nProduct, architecture, protocol, security, roadmap"]
  Tools["tools + tests + CI\nPackaging, diagnostics, GitHub Actions"]

  Root --> Android
  Root --> Windows
  Root --> Mac
  Root --> Crates
  Root --> Docs
  Root --> Tools
```

| Path | Purpose | Current State |
| --- | --- | --- |
| `apps/android-client` | Android tablet/phone client | Compose UI, LAN discovery, stylus diagnostics, live stylus UDP sender, MediaCodec decode surface |
| `hosts/windows-host` | Primary desktop host | LAN backend runtime, UDP routing, GDI capture fallback, encoder abstraction, Win32 synthetic pen injection wrapper |
| `hosts/macos-host` | Secondary desktop host | SwiftUI shell, ScreenCaptureKit display enumeration, VideoToolbox encoder foundation |
| `crates/core` | Shared math and state | Coordinate mapping, calibration, pressure curves, session state |
| `crates/protocol` | Binary protocol | `GLYR` frames and compact `GLYS` stylus batches |
| `crates/transport` | Real-time packet layer | UDP `GLYT`, LAN discovery `GLYD`, video fragmentation, secure datagram wrapper, reconnect, bitrate logic |
| `crates/security` | Pairing/session primitives | Pairing codes, HMAC challenge response, session cipher, replay guard, secret-store traits |
| `crates/telemetry` | Local diagnostics | Latency breakdowns and rolling metrics |
| `crates/audio` | Audio foundation | Audio packetization primitives |
| `docs` | Product knowledge base | Architecture, security, Windows Ink, Android stylus, macOS, test plan, performance targets |

## System Shape

```mermaid
flowchart LR
  subgraph Client["Android Client"]
    UI["Compose UI"]
    Stylus["MotionEvent stylus capture"]
    Decode["MediaCodec H.264 decode"]
    Discovery["LAN discovery receiver"]
  end

  subgraph Shared["Rust Shared Layer"]
    Protocol["Protocol\nGLYR / GLYS"]
    Transport["Transport\nGLYT / GLYD / fragments"]
    Security["Security\npairing, auth, cipher"]
    Core["Core\nmapping, pressure, calibration"]
  end

  subgraph Host["Windows Host"]
    Backend["Backend runtime"]
    Capture["Screen capture"]
    Encode["H.264 encoder abstraction"]
    Ink["Windows Ink synthetic pen injection"]
  end

  Discovery --> Transport
  Stylus --> Protocol
  UI --> Core
  Decode --> Transport
  Protocol <--> Transport
  Transport <--> Backend
  Security <--> Backend
  Backend --> Capture
  Capture --> Encode
  Backend --> Ink
```

## Runtime Data Flow

```mermaid
sequenceDiagram
  participant A as Android Client
  participant T as UDP Transport
  participant H as Windows Host
  participant W as Windows Ink

  H->>A: GLYD LAN host advertisement
  A->>H: Pairing/control packets
  A->>T: GLYS stylus batch wrapped in GLYT
  T->>H: High-priority input datagram
  H->>W: CreateSyntheticPointerDevice / InjectSyntheticPointerInput
  H-->>A: Latency pong / session status
  H-->>A: Video fragments when live stream is enabled
```

## Implemented Highlights

- Rust workspace for shared core, protocol, transport, security, telemetry, and audio.
- Versioned binary protocol with stylus, media, session, pairing, latency, and control messages.
- Compact high-frequency stylus wire format (`GLYS`) shared by Android and Rust.
- Coordinate mapping, calibration, and pressure-curve logic with unit tests.
- Android Compose app with host list, pairing, connection, session, pen settings, video settings, security, and diagnostics screens.
- Android raw stylus diagnostics for pressure, tilt, orientation, hover, buttons, eraser, history, and timestamps.
- Android LAN host discovery receiver for Rust `GLYD` advertisements.
- Android remote-session stylus bridge that captures drawing-surface input and sends compact stylus batches over UDP on a background worker.
- Android low-latency `SurfaceView` and `MediaCodec` H.264 decoder foundation.
- Windows backend runtime with LAN discovery, UDP server routing, session registry, pairing request handling, permission gating, and latency pong replies.
- Windows development auto-approval mode for local LAN stylus-path smoke testing.
- Windows backend opt-in native pen injection bridge for LAN smoke tests.
- Windows stylus input bridge and Win32 synthetic pen injection wrapper.
- Windows monitor enumeration, GDI capture fallback, encoder abstraction, and streaming pipeline shape.
- ChaCha20-Poly1305 session cipher, replay guard, secure datagram codec, reconnect, and adaptive bitrate foundations.
- macOS SwiftUI shell with ScreenCaptureKit, VideoToolbox, CGEvent, and audio permission foundations.
- GitHub Actions CI for Rust tests and Android debug build.

## Build And Run

### Rust Workspace

Install stable Rust, then run:

```powershell
cargo test --workspace
```

Run Windows diagnostics:

```powershell
cargo run -p glyphray-pen-diagnostics
cargo run -p glyphray-capture-diagnostics
cargo run -p glyphray-host-diagnostics
```

Run the Windows backend runtime:

```powershell
cargo run -p glyphray-windows-host -- serve
```

For local input-path smoke testing before the host approval UI exists:

```powershell
$env:GLYPHRAY_DEV_AUTO_APPROVE='1'
$env:GLYPHRAY_ENABLE_PEN_INJECTION='1'
cargo run -p glyphray-windows-host -- serve
```

### Android Client

Install Android Studio or Android SDK command-line tools, then run:

```powershell
.\gradlew.bat :apps:android-client:assembleDebug
```

The Android app currently includes LAN host discovery, stylus diagnostics, a session UI, a latency overlay, a remote-session stylus UDP bridge, and a MediaCodec-backed decoder surface prepared for incoming H.264 frames.

### macOS Host

On macOS 13+ with Xcode installed:

```bash
cd hosts/macos-host
swift build
```

The macOS host is still a Phase 2/5 foundation. Windows remains the primary platform for native pen injection.

## Important Documents

| Document | Why It Matters |
| --- | --- |
| [docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md) | Product goals, target users, and constraints |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System boundaries and component diagrams |
| [docs/PROTOCOL.md](docs/PROTOCOL.md) | Binary protocol and message shape |
| [docs/SECURITY.md](docs/SECURITY.md) | Threat model and security requirements |
| [docs/WINDOWS_INK_INJECTION.md](docs/WINDOWS_INK_INJECTION.md) | Windows native pen injection notes |
| [docs/ANDROID_STYLUS_CAPTURE.md](docs/ANDROID_STYLUS_CAPTURE.md) | Android stylus capture and packetization |
| [docs/BACKEND.md](docs/BACKEND.md) | Windows backend runtime notes |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestone checklist |
| [docs/TEST_PLAN.md](docs/TEST_PLAN.md) | Validation plan |
| [docs/PERFORMANCE_TARGETS.md](docs/PERFORMANCE_TARGETS.md) | Latency and telemetry targets |
| [docs/DEVELOPMENT_DIARY.md](docs/DEVELOPMENT_DIARY.md) | Running development diary |

## Current Limits

- Full local builds were not executed in this workspace because `cargo`, `gradle`, `swift`, and Android SDK tools are not installed on `PATH`.
- Windows capture currently has a GDI fallback; production should move to Windows Graphics Capture or Desktop Duplication.
- A concrete H.264 hardware/software encoder backend still needs to replace the placeholder abstraction.
- Android stylus packets can be captured from the remote display surface and sent over UDP, but the complete production pairing and session handshake still needs hardening.
- Host approval UI is not wired yet; `GLYPHRAY_DEV_AUTO_APPROVE` is only for local smoke tests.
- `GLYPHRAY_ENABLE_PEN_INJECTION` uses temporary 1920x1080 stretch mapping until display negotiation and calibration are fully wired.
- Native Windows Ink pressure/tilt/hover must still be validated in real creative apps.

## Next Focus

```mermaid
flowchart LR
  A["Host approval UI"] --> B["Secure session handshake"]
  B --> C["Android stylus stream over LAN"]
  C --> D["Native Windows Ink validation"]
  D --> E["Live video encode/send loop"]
  E --> F["Packaging and beta readiness"]
```

Immediate engineering focus:

- Wire Android-selected hosts into the real pairing/control channel.
- Connect Android stylus UDP packets to the Windows native pen bridge in a full LAN smoke test.
- Replace fallback capture with Windows Graphics Capture or Desktop Duplication.
- Add a concrete low-latency H.264 encoder backend.
- Drive the video streaming pipeline continuously from the backend runtime.
