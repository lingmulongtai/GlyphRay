# GlyphRay

[日本語版 README](README.ja.md)

GlyphRay is a low-latency remote creative desktop application for artists, designers, and illustrators who want to use an Android tablet or phone as a high-quality remote pen display for a Windows or macOS computer.

The product goal is Parsec-like speed and simplicity with an original brand, UI, codebase, protocol, and architecture. The key differentiator is native Windows Ink-style pen injection from Android stylus input, not mouse-only emulation.

## Current Progress

**Overall progress estimate: 91%**

Last updated: 2026-05-15 JST

```mermaid
pie title Overall Completion
  "Implemented foundation" : 91
  "Remaining product work" : 9
```

| Area | Status | Progress |
| --- | --- | ---: |
| Milestone 1 foundation | Complete | 100% |
| Milestone 2 video and transport foundation | In progress | 92% |
| Milestone 3 Android stylus to Windows Ink stream | In progress | 89% |
| Milestone 4 hardening and packaging | In progress | 80% |
| Milestone 5 macOS, audio, relay readiness | In progress | 42% |

```text
M1 Foundation                 [####################] 100%
M2 Video + Transport          [##################--]  92%
M3 Stylus -> Windows Ink      [##################--]  89%
M4 Security + Packaging       [################----]  80%
M5 macOS + Audio + Relay      [########------------]  42%
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
  Website["website/*\nGitHub Pages download site"]
  Tools["tools + tests + CI\nPackaging, diagnostics, GitHub Actions"]

  Root --> Android
  Root --> Windows
  Root --> Mac
  Root --> Crates
  Root --> Docs
  Root --> Website
  Root --> Tools
```

| Path | Purpose | Current State |
| --- | --- | --- |
| `apps/android-client` | Android tablet/phone client | Compose UI, LAN discovery, control handshake send/receive, stylus diagnostics, live stylus UDP sender, MediaCodec decode surface |
| `hosts/windows-host` | Primary desktop host | LAN backend runtime, UDP routing, QoS outbound queues, approved-peer video fragment queueing, health/status metrics, pending-peer hardening, native permission dialog, GDI capture fallback, encoder abstraction, Win32 synthetic pen injection wrapper |
| `hosts/macos-host` | Secondary desktop host | SwiftUI shell, ScreenCaptureKit display enumeration, permission readiness UI, Keychain smoke test, VideoToolbox encoder smoke test |
| `crates/core` | Shared math and state | Coordinate mapping, calibration, pressure curves, session state |
| `crates/protocol` | Binary protocol | `GLYR` frames and compact `GLYS` stylus batches |
| `crates/transport` | Real-time packet layer | UDP `GLYT`, LAN discovery `GLYD`, video fragmentation, reusable UDP buffers, secure datagram wrapper, reconnect, bitrate/keyframe adaptation logic |
| `crates/security` | Pairing/session primitives | Pairing codes, HMAC challenge response, session cipher, replay guard, secret-store traits |
| `crates/telemetry` | Local diagnostics | Latency breakdowns and rolling metrics |
| `crates/audio` | Audio foundation | Audio packetization primitives |
| `docs` | Product knowledge base | Architecture, security, Windows Ink, Android stylus, macOS, test plan, performance targets |
| `website` | GitHub Pages site | Static download page, generated hero image, release links, setup command generator |

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
  A->>H: GLYR PairingRequest / LatencyPing over GLYT control
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
- Android Compose app with polished host list, pairing, connection readiness, session cockpit, pen settings, video settings, security, and diagnostics screens.
- Android raw stylus diagnostics for pressure, tilt, orientation, hover, buttons, eraser, history, and timestamps.
- Android LAN host discovery receiver for Rust `GLYD` advertisements.
- Android control channel sender for `PairingRequest` and `LatencyPing` frames wrapped in `GLYT`.
- Android control response receiver for `PairingResult` and `LatencyPong`.
- Android display-info receiver for host monitor geometry after pairing.
- Android video settings can select a discovered host display, and the selected display id is sent with stylus, touch, and mouse input packets.
- Android video/session settings for resolution, refresh rate, bitrate, color space, codec, touch mode, fullscreen mode, Bluetooth keyboard/mouse capture, game controller capture, and special-key overlay.
- Android persists video and input preferences so stream quality, touch mode, capture toggles, and fullscreen preference survive app restarts.
- Android session fullscreen now hides system bars with immersive mode and keeps the screen awake during active sessions.
- Android manual host entry for Tailscale IP / MagicDNS / direct endpoint use, with saved endpoints restored into the host list.
- Android remote-session input bridge that sends stylus, native touch, Bluetooth mouse, keyboard, and gamepad packets over UDP on QoS-aware background workers.
- Android touch modes now include direct native touch, trackpad-style cursor movement, and two-finger gesture wheel translation.
- Android realtime receive path can route `VideoFrame` packets from the transport socket into `RemoteVideoStreamController` and the MediaCodec decoder.
- Android low-latency `SurfaceView` and `MediaCodec` H.264 decoder foundation.
- Windows backend runtime with LAN discovery, UDP server routing, session registry, pairing request handling, console approval/rejection, optional native permission dialogs, `PairingResult`, display-info responses, encoder config intake, opt-in keyboard/mouse/touch injection, gamepad decode, permission gating, and latency pong replies.
- Windows backend hardening for pending-session caps, per-IP pending attempt rate limiting, late input packet dropping, channel-aware nonblocking QoS outbound queues, approved-peer video fragment queueing, and console-visible queue/backpressure health metrics.
- Windows host video pump can restart from approved client `EncoderConfig` and has a console `encoder override` command for host-side stream control.
- Windows host can persist a default encoder override with `encoder save`, reload it on backend startup, clear it with `encoder clear`, and manage named stream presets with `encoder preset save|apply|delete|list`.
- Windows host can show an opt-in Win32 connection permission dialog for incoming pairing requests with `GLYPHRAY_ENABLE_PERMISSION_DIALOG=1`.
- Windows host supports per-user startup-at-login management with `startup status`, `startup enable`, and `startup disable`.
- Windows runtime input bridges now derive their mapper from the selected display geometry instead of a fixed smoke-test rectangle when the display can be enumerated.
- Windows stylus bridge now normalizes pen axes and smooths pressure before calling the native Win32 synthetic pen injector.
- Windows development auto-approval mode for local LAN stylus-path smoke testing.
- Windows backend opt-in native pen injection bridge for LAN smoke tests.
- Windows stylus input bridge and Win32 synthetic pen injection wrapper.
- Windows monitor enumeration, GDI capture fallback, encoder abstraction, and streaming pipeline shape.
- ChaCha20-Poly1305 session cipher, replay guard, secure datagram codec, reconnect, adaptive bitrate decisions, and packet-loss keyframe recovery signaling foundations.
- Windows `PlatformSecretStore` uses DPAPI-protected per-user secret files on Windows, with an in-memory fallback on non-Windows builds.
- macOS SwiftUI shell with ScreenCaptureKit display diagnostics, permission readiness checks, VideoToolbox low-latency encoder smoke test, CGEvent mouse/keyboard foundation, Keychain secret-store smoke test, and audio permission plumbing.
- GitHub Actions CI for Rust tests, Android unit tests, and Android debug build.
- GitHub Pages static download site with setup command generator and original hero artwork.

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

While the backend is running, `encoder status`, `encoder override 1920x1080 120 35000`, `encoder save`, `encoder preset save studio-120`, `encoder preset apply studio-120`, `encoder preset delete studio-120`, and `encoder clear` are available in the host console for stream-control smoke tests. `encoder save` persists the active host override, or the latest approved client `EncoderConfig`, and reloads it on the next backend start. Named presets are stored alongside the default override for quick 60fps/120fps/bitrate switching during hardware validation.

Manage user-logon startup:

```powershell
cargo run -p glyphray-windows-host -- startup status
cargo run -p glyphray-windows-host -- startup enable
cargo run -p glyphray-windows-host -- startup disable
```

For native host-side pairing approval during LAN tests:

```powershell
$env:GLYPHRAY_ENABLE_PERMISSION_DIALOG='1'
cargo run -p glyphray-windows-host -- serve
```

For local input-path smoke testing when you deliberately want to bypass approval:

```powershell
$env:GLYPHRAY_DEV_AUTO_APPROVE='1'
$env:GLYPHRAY_ENABLE_PEN_INJECTION='1'
$env:GLYPHRAY_ENABLE_TOUCH_INJECTION='1'
$env:GLYPHRAY_ENABLE_MOUSE_INJECTION='1'
$env:GLYPHRAY_ENABLE_KEYBOARD_INJECTION='1'
$env:GLYPHRAY_ENABLE_VIDEO_STREAM='1'
cargo run -p glyphray-windows-host -- serve
```

### Android Client

Install Android Studio or Android SDK command-line tools, then run:

```powershell
.\gradlew.bat :apps:android-client:assembleDebug
```

The Android app currently includes LAN host discovery, stylus diagnostics, a session UI, a latency overlay, a remote-session stylus UDP bridge, and a MediaCodec-backed decoder surface prepared for incoming H.264 frames.

JDK 17 remains the safest baseline for Gradle/Android work. The wrapper is pinned to Gradle 8.14.3 so Java 24 local test runs are also supported, though the JVM may print native-access warnings.

```powershell
.\gradlew.bat :apps:android-client:testDebugUnitTest
```

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
| [docs/WINDOWS_STARTUP_AND_SERVICE.md](docs/WINDOWS_STARTUP_AND_SERVICE.md) | Startup-at-login implementation and service/agent constraints |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestone checklist |
| [docs/TEST_PLAN.md](docs/TEST_PLAN.md) | Validation plan |
| [docs/PERFORMANCE_TARGETS.md](docs/PERFORMANCE_TARGETS.md) | Latency and telemetry targets |
| [docs/FEATURE_MATRIX.md](docs/FEATURE_MATRIX.md) | Implementation status for video, input, fullscreen, special keys, and host startup |
| [docs/NETWORK_COMPATIBILITY.md](docs/NETWORK_COMPATIBILITY.md) | LAN, Tailscale, and overlay VPN notes |
| [docs/RELEASE_DISTRIBUTION.md](docs/RELEASE_DISTRIBUTION.md) | Windows/macOS installers and Play Store release path |
| [docs/DEVELOPMENT_DIARY.md](docs/DEVELOPMENT_DIARY.md) | Running development diary |

## Website

The GitHub Pages site lives in [website](website). It is frontend-only and can be opened directly:

```powershell
Start-Process .\website\index.html
```

Deployment is handled by [pages.yml](.github/workflows/pages.yml). Enable Pages once in repository settings and choose GitHub Actions as the source before rerunning the workflow. GitHub does not always allow the workflow token to create the Pages site automatically.

## Current Limits

- Rust tests and Android debug builds have been exercised on Windows. Android unit tests should be run with JDK 17.
- The host router now has in-memory DoS guards, console-visible health counters, and an opt-in native permission dialog for pending peer approval. Production pairing still needs persistent trusted-device management.
- Windows capture currently has a GDI fallback; production should move to Windows Graphics Capture or Desktop Duplication.
- Video fragments can now be queued to approved clients on the Video channel, but a concrete H.264 hardware/software encoder backend still needs to replace the placeholder abstraction before real desktop video is useful.
- Android stylus packets can be captured from the remote display surface and sent over UDP, but the complete production pairing and session handshake still needs hardening.
- The permission dialog is a minimal Win32 prompt, not a full tray/settings UI yet. `GLYPHRAY_DEV_AUTO_APPROVE` remains only for local smoke tests.
- `GLYPHRAY_ENABLE_PEN_INJECTION` uses temporary 1920x1080 stretch mapping until display negotiation and calibration are fully wired.
- `GLYPHRAY_ENABLE_TOUCH_INJECTION`, `GLYPHRAY_ENABLE_MOUSE_INJECTION`, and `GLYPHRAY_ENABLE_KEYBOARD_INJECTION` are explicit smoke-test flags until per-device input permissions are exposed in the host UI.
- Gamepad packets are decoded on Windows, but virtual controller injection still needs a ViGEm/virtual HID backend.
- Native Windows Ink pressure/tilt/hover must still be validated in real creative apps.

## Next Focus

```mermaid
flowchart LR
  A["Tray settings UI"] --> B["Secure session handshake"]
  B --> C["Android stylus stream over LAN"]
  C --> D["Native Windows Ink validation"]
  D --> E["Live video encode/send loop"]
  E --> F["Packaging and beta readiness"]
```

Immediate engineering focus:

- Promote the native permission dialog into a tray/settings UI with persistent trusted-device controls.
- Connect Android stylus UDP packets to the Windows native pen bridge in a full LAN smoke test.
- Replace fallback capture with Windows Graphics Capture or Desktop Duplication.
- Add a concrete low-latency H.264 encoder backend.
- Drive the video streaming pipeline continuously from the backend runtime.
