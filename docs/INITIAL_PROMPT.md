# Initial GlyphRay Build Prompt

This file preserves the initial product/development prompt used to start the repository.

```text
You are a senior product engineer, systems architect, Android engineer, Windows native engineer, macOS engineer, and security engineer.

Build a product-grade monorepo for a low-latency remote creative desktop application.

Working product name:
GlyphRay

Important legal/product constraint:
Do NOT copy Parsec’s source code, UI assets, brand, icons, proprietary protocol, wording, or exact visual design. Use Parsec only as a benchmark for user experience, latency, simplicity, and performance. Implement an original product with original UI, original architecture, and original branding.

Product concept:
GlyphRay is a Parsec-like low-latency remote desktop app focused on digital artists, designers, illustrators, and people who want to use an Android tablet or phone as a high-quality remote pen display for a Windows or macOS computer.

The main differentiator:
Android stylus input, especially Samsung S Pen input from a Samsung Galaxy Tab S11 Ultra, must be transmitted to the host and injected on Windows as real Windows Ink / native pen input, not just mouse input.

The goal:
Create an Android client and Windows/macOS host system in one repository.
The experience should feel as simple, smooth, secure, and low-latency as Parsec, but the product must focus on pen support for creative apps.

Target platforms:
1. Android client
   - Primary test device: Samsung Galaxy Tab S11 Ultra
   - Also support Android phones and tablets where possible

2. Windows host
   - Windows 10/11
   - Primary priority
   - Must support Windows Ink-compatible pen injection

3. macOS host
   - Secondary priority
   - macOS 13+
   - Capture and stream the screen using native APIs
   - Input support can start with mouse/keyboard compatibility, then improve later

Repository structure:
Create a single monorepo with this structure:

/apps/android-client
/hosts/windows-host
/hosts/macos-host
/crates/core
/crates/protocol
/crates/transport
/crates/security
/crates/telemetry
/docs
/tools
/tests
/.github/workflows

Recommended languages and frameworks:
- Shared core/protocol/transport/security: Rust
- Android client: Kotlin + Jetpack Compose + Android NDK where needed
- Windows host: Rust + Win32 API bindings, with small C++ modules only if required
- macOS host: Swift/SwiftUI for native shell, Rust shared core, ScreenCaptureKit, VideoToolbox
- Build tools: Gradle for Android, Cargo for Rust, Xcode project or Swift Package for macOS
- CI: GitHub Actions

Do not create a fake prototype only. Create a real, buildable foundation with clean architecture, tests, and documentation.

Core requirements:

1. Android client
Implement:
- Clean modern UI using Jetpack Compose
- Dark mode first, simple and professional design
- Host discovery screen
- Pairing screen
- Connection screen
- Remote display view
- Latency/status overlay
- Settings screen

Android input requirements:
- Capture stylus MotionEvent data
- Capture:
  - x/y coordinates
  - pressure
  - tilt
  - orientation
  - hover state
  - button state
  - eraser if available
  - tool type
  - historical batched points
  - event timestamps
- Preserve high-frequency stylus data
- Add palm rejection handling where possible
- Add pressure curve settings:
  - linear
  - soft
  - hard
  - custom curve later
- Add coordinate mapping:
  - fit
  - fill
  - 1:1
  - selected monitor
  - drawing area calibration
- Add a stylus diagnostics screen that shows raw stylus values in real time

Android video requirements:
- Use hardware decoding where possible
- Use MediaCodec for H.264 first
- Prepare architecture for H.265 and AV1 later
- Prioritize low latency over buffering
- Render video to a low-latency SurfaceView or equivalent
- Support 60fps first
- Prepare for 90/120fps later on compatible devices

2. Windows host
Implement:
- Native Windows desktop app/service
- System tray or minimal host UI
- Pairing flow
- Connection permission dialog
- Monitor selection
- Encoder settings
- Security settings
- Diagnostic screen

Windows screen capture:
- Use Windows Graphics Capture or Desktop Duplication API
- Prefer the most stable low-latency implementation
- Capture selected monitor first
- Prepare for window capture later
- Capture cursor optionally

Windows encoding:
- Use hardware encoding where possible
- Start with H.264
- Design abstraction for:
  - Intel Quick Sync
  - NVIDIA NVENC
  - AMD AMF
  - software fallback
- Low-latency encoder settings:
  - no B-frames
  - low-latency preset
  - adaptive bitrate
  - keyframe interval control
  - resolution scaling

Windows input injection:
This is the most important feature.

Implement real pen injection using Windows pointer APIs.
The Android stylus input must become native Windows pen input, not mouse movement.

Requirements:
- Use CreateSyntheticPointerDevice and InjectSyntheticPointerInput or the correct modern Win32 pointer input injection path.
- Use PT_PEN / POINTER_TYPE_INFO / POINTER_PEN_INFO where applicable.
- Preserve:
  - x/y location
  - pressure, normalized to Windows pen pressure range
  - tiltX
  - tiltY
  - rotation/orientation where possible
  - barrel button / side button
  - hover
  - pen down / move / up
  - eraser mode if available
- Add fallback to mouse injection only when pen injection is unavailable.
- Clearly document limitations.
- Add an input test utility that visualizes whether Windows receives pressure, tilt, hover, and pen state.
- Test target apps:
  - Clip Studio Paint
  - Adobe Photoshop
  - Krita
  - OneNote
  - Blender Grease Pencil if possible
  - Figma / design tools where relevant

Coordinate mapping:
- Android client coordinates must map correctly to Windows display coordinates.
- Support multi-monitor.
- Support high DPI scaling.
- Support display rotation.
- Support aspect ratio differences.
- Add calibration mode.

3. macOS host
Implement as Phase 2 but prepare structure now:
- Native macOS host app
- Screen capture using ScreenCaptureKit
- Encoding using VideoToolbox
- Input injection using CGEvent where possible
- Document that native Windows Ink-style pen injection is Windows-specific
- Keep architecture parallel with Windows host

4. Networking and transport
Goal:
Very low-latency remote desktop transport similar in feel to Parsec.

For the first implementation:
- Use a secure low-latency transport suitable for real-time video and input
- WebRTC is acceptable for MVP if it gives us DTLS/SRTP, NAT traversal, and hardware media pipeline compatibility
- If using WebRTC, keep the transport abstraction clean so a custom UDP/RTP-like transport can be implemented later
- Use UDP-based transport for real-time video/input, not TCP-only streaming
- Prioritize:
  - low latency
  - jitter handling
  - packet loss recovery
  - adaptive bitrate
  - NAT traversal
  - reconnect handling

Transport requirements:
- Video stream channel
- Audio stream channel, can be Phase 2
- Input event channel with high priority
- Control channel
- Latency ping/pong
- Bandwidth estimation
- Connection quality telemetry

Input data must be prioritized over video when needed.
Stylus packets should be tiny, frequent, and timestamped.

5. Protocol
Create a versioned binary protocol.

Define messages such as:
- ClientHello
- HostHello
- AuthChallenge
- AuthResponse
- PairingRequest
- PairingResult
- DisplayInfo
- EncoderConfig
- VideoFrame
- AudioFrame
- StylusInputBatch
- MouseInput
- KeyboardInput
- ClipboardMessage, optional later
- LatencyPing
- LatencyPong
- ErrorMessage
- Disconnect

StylusInputBatch must support multiple samples per packet:
- sequence number
- monotonic timestamp
- display id
- pointer id
- tool type
- action
- x
- y
- pressure
- tilt
- orientation
- button flags
- hover flag
- eraser flag
- predicted flag if used later

Use a schema system such as FlatBuffers, Cap’n Proto, or a carefully documented custom binary format.
Do not use JSON for high-frequency input/video data.

6. Security
Security must be treated as product-level, not demo-level.

Requirements:
- Secure pairing
- Device identity keys
- Per-device trusted host list
- QR code pairing or numeric pairing code
- Mutual authentication
- Encrypted transport
- No plaintext input data over the network
- No plaintext long-term secrets on disk
- Android Keystore for Android secrets
- Windows DPAPI or Credential Manager for Windows secrets
- macOS Keychain for macOS secrets
- Session tokens must be short-lived
- One-time pairing tokens
- Rate limiting for pairing attempts
- Clear permission model
- Local network mode first
- Cloud account system is NOT required for MVP
- Relay server is NOT required for MVP, but architecture should allow it later
- Add SECURITY.md
- Add threat model document

Threats to consider:
- Unauthorized host access
- Man-in-the-middle attacks
- Replay attacks
- Stolen pairing code
- Malicious LAN device
- Input injection abuse
- Host privilege boundaries
- Logging sensitive input accidentally
- Clipboard leakage

Never log raw keyboard input by default.
Never log passwords or raw typed text.
Stylus diagnostics may log stylus values only when explicitly enabled.

7. Performance targets
Add benchmark tools and telemetry.

Initial targets:
- LAN 1080p60 glass-to-glass latency: target under 35 ms p95
- LAN stylus input-to-host-injection latency: target under 8 ms after packet arrival
- Stable 60fps
- Minimal frame pacing jitter
- Fast reconnect
- Adaptive bitrate under packet loss
- CPU/GPU usage should be visible in diagnostics

Add tools to measure:
- encode time
- network time
- decode time
- render time
- input capture time
- input transport time
- input injection time
- end-to-end estimated latency

8. UI/UX
Do not clone Parsec’s UI exactly.
Create an original clean interface.

Design principles:
- Simple
- Fast
- Dark mode first
- Creator-focused
- Minimal setup
- Obvious connect/disconnect state
- Clear latency and connection quality display
- Pen settings easy to access
- No unnecessary social/gaming features

Android screens:
- Welcome
- Pair new computer
- Host list
- Connect screen
- Remote session screen
- Pen settings
- Video settings
- Security settings
- Diagnostics

Windows/macOS host screens:
- Status
- Pair device
- Connected clients
- Display settings
- Encoder settings
- Security settings
- Diagnostics

9. Documentation
Create:
- README.md
- docs/PRODUCT_SPEC.md
- docs/ARCHITECTURE.md
- docs/PROTOCOL.md
- docs/SECURITY.md
- docs/WINDOWS_INK_INJECTION.md
- docs/ANDROID_STYLUS_CAPTURE.md
- docs/MACOS_HOST.md
- docs/ROADMAP.md
- docs/TEST_PLAN.md
- docs/PERFORMANCE_TARGETS.md

10. Testing
Add:
- Unit tests for protocol serialization/deserialization
- Unit tests for coordinate mapping
- Unit tests for pressure curve mapping
- Unit tests for authentication/session logic
- Windows input injection test tool
- Android stylus diagnostics screen
- Transport simulation tests for latency, jitter, and packet loss
- Basic CI pipeline

11. Development phases
Do not try to finish the entire commercial product in one huge messy implementation.
Work in clear milestones.

Milestone 1:
- Create monorepo
- Create architecture docs
- Create protocol crate
- Create Android client skeleton
- Create Windows host skeleton
- Implement Android stylus diagnostics
- Implement Windows pen injection test harness
- Implement local LAN pairing skeleton
- Implement coordinate mapping and pressure mapping tests

Milestone 2:
- Implement screen capture on Windows
- Implement hardware H.264 encoding path or clean abstraction
- Implement Android hardware decoding skeleton
- Send video over LAN
- Add latency overlay

Milestone 3:
- Connect Android stylus stream to Windows native pen injection
- Validate pressure/tilt/hover in Windows creative apps
- Add calibration UI

Milestone 4:
- Harden security
- Add reconnect
- Add adaptive bitrate
- Add diagnostics
- Add installer packaging

Milestone 5:
- Add macOS host
- Add audio
- Add relay architecture if needed
- Prepare beta release

12. What to implement now
Assume the repository is empty.

In this first Codex run, implement Milestone 1 as much as possible with real code.

Deliverables for this run:
- Buildable monorepo skeleton
- Rust workspace with protocol, core, security, and transport crates
- Protocol definitions for stylus input and session control
- Unit tests for protocol and coordinate/pressure mapping
- Android Jetpack Compose project skeleton
- Android stylus diagnostics screen that reads MotionEvent stylus data
- Windows host Rust project skeleton
- Windows pen injection module or wrapper with clear API
- Windows diagnostic tool that can inject synthetic pen events from test data
- Documentation files listed above with meaningful initial content
- GitHub Actions CI for Rust tests and basic Android build if possible
- README with setup instructions

Implementation quality rules:
- Prefer real implementations over stubs
- If a platform API is difficult, create a clearly isolated module and document exactly what remains
- Do not hide TODOs in code without tracking them in docs/ROADMAP.md
- Keep code readable and modular
- Add comments only where they explain non-obvious system behavior
- Make error handling explicit
- Do not use unsafe Rust unless necessary; if used, isolate and document it
- Do not include secrets, API keys, or certificates in the repo
- Do not add telemetry that sends data to external servers
- Do not depend on a commercial backend for MVP

Output expectation:
After making changes, summarize:
1. What was implemented
2. How to build each component
3. What tests exist
4. What remains for Milestone 2
5. Any platform limitations discovered
```

