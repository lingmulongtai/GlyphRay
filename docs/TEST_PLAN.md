# Test Plan

## Unit Tests

Rust tests cover:

- Protocol frame encode/decode.
- Protocol checksum validation.
- Coordinate mapping modes.
- Display rotation mapping.
- Pressure curve mapping.
- Challenge-response authentication helpers.
- Pairing rate limiting.
- Transport packet priority.
- UDP transport datagram encode/decode and checksum rejection.
- Video frame fragment/reassembly behavior.
- Encoded video access-unit packetization and reassembly.
- LAN discovery advertisement encode/decode.
- Backend session permission gate and pairing request routing.
- Backend pending-session cap and oldest-pending eviction.
- Backend per-IP pending attempt rate limiting.
- Backend late input packet drop before injection.
- Backend outbound QoS queue priority for control over video backlog.
- Backend outbound queue snapshot lengths, drop counters, and high watermark.
- Backend approved-peer video fragment queueing on the Video channel.
- Backend stable CRC-based host id generation.
- Stylus bridge pressure smoothing and pen-axis normalization before native injection.
- Backend stylus packet routing into the input bridge.
- Transport packet-loss simulation.
- Telemetry p95 and latency totals.
- Windows host encoder settings and start-state behavior.
- Session cipher sealing/opening and AAD authentication.
- Replay guard duplicate counter rejection.
- Reconnect backoff.
- Adaptive bitrate response to loss/jitter.
- Audio packetization.
- Relay candidate selection.

Run:

```powershell
cargo test --workspace
```

## Android Manual Tests

- Launch app on Samsung Galaxy Tab S11 Ultra.
- Open Diagnostics.
- Hover S Pen over the diagnostic area.
- Confirm hover action and distance update.
- Draw with light and heavy pressure.
- Confirm historical sample count increases during fast strokes.
- Confirm eraser tool type when the device exposes it.
- Confirm button state changes with barrel/side button.
- Feed a known H.264 Annex B access-unit sequence through `RemoteVideoStreamController`.
- Confirm the `SurfaceView` shows decoded video and the latency overlay remains responsive.
- Confirm Video-channel `VideoFrame` datagrams are routed into `RemoteVideoStreamController` while control responses still update pairing/latency state.
- Confirm the Android QoS queue prioritizes input/control ahead of a video backlog.
- Confirm compact stylus packets decode correctly on the host.
- Confirm calibration target flow can be operated without layout overlap.
- Add a host manually by Tailscale IP or MagicDNS name and confirm pairing reaches the host.
- Confirm Bluetooth mouse movement, buttons, and wheel produce `MouseInput` packets.
- Confirm Android-connected gamepad buttons and sticks produce `GamepadInput` packets.

## Windows Manual Tests

- Run `cargo run -p glyphray-pen-diagnostics`.
- Open Krita or OneNote and focus a canvas.
- Confirm a synthetic stroke appears.
- Confirm pressure varies across the stroke.
- Confirm hover/down/up transitions in a pointer inspection tool.
- Run a host capture smoke test once the capture CLI is added.
- Run `cargo run -p glyphray-capture-diagnostics`.
- Run `cargo run -p glyphray-host-diagnostics`.
- Run `cargo run -p glyphray-windows-host -- serve`.
- Confirm GDI fallback captures the selected monitor before replacing it with Windows Graphics Capture or Desktop Duplication.
- Confirm a pending Android peer cannot inject input until approved.
- Confirm spoofed/random source ports cannot grow pending sessions beyond the configured cap.
- Confirm one source IP rotating ports is rate-limited before it can evict all other pending peers.
- Run `status` in the host console and confirm pending sessions, outbound queue depth, drops, and backpressure counters are visible.
- Confirm delayed lower-sequence stylus/touch/mouse/keyboard packets are dropped instead of injected.
- With `GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1`, confirm an approved Android Bluetooth keyboard can inject a safe key sequence.
- Confirm Win and PrintScreen overlay keys are blocked until the host peer is approved and keyboard injection is explicitly enabled.
- With `GLYPHRAY_ENABLE_TOUCH_INJECTION=1`, confirm Android finger input is received by Windows as native touch input in a touch-aware app.
- With `GLYPHRAY_ENABLE_MOUSE_INJECTION=1`, confirm Bluetooth mouse movement/buttons/wheel are injected after approval.
- Confirm gamepad packets are decoded, then validate virtual-controller injection once ViGEm/virtual HID backend exists.

## macOS Manual Tests

- Build the SwiftPM host on macOS 13+ with `swift build`.
- Launch the host and confirm Screen Recording, Accessibility, Input Monitoring, and audio readiness states are visible.
- Use the Screen Recording and Accessibility request buttons and confirm macOS opens the expected permission prompts.
- Confirm ScreenCaptureKit display listing shows each available display with geometry.
- Run the VideoToolbox encoder smoke test and confirm a low-latency H.264 session can be created.
- Run the Keychain smoke test and confirm save/load/delete passes before wiring device identity.
- Run the Windows DPAPI `PlatformSecretStore` round-trip test and confirm secrets survive reopening the store.
- Run `glyphray-windows-host startup status`, then enable and disable startup on a disposable Windows test user and confirm the HKCU Run value changes as expected.
- Run `encoder override 1920x1080 120 35000`, `encoder save`, restart the backend, and confirm `encoder status` reports the saved override before any client connects.
- Run `encoder preset save studio-120`, `encoder preset list`, `encoder preset apply studio-120`, and `encoder preset delete studio-120`; confirm apply restarts the video pump when `GLYPHRAY_ENABLE_VIDEO_STREAM=1` is set and delete removes only the named preset, not the saved default override.
- Post a safe CGEvent mouse move/click and keyboard test only after Accessibility permission is granted.

## Integration Tests To Add

- Android-to-host stylus batch replay.
- Android-to-host Bluetooth keyboard replay with Windows virtual-key mapping.
- Android-to-host native touch replay.
- Android-to-host Bluetooth mouse replay.
- Android-to-host gamepad replay.
- Host coordinate mapping under multi-monitor DPI layouts.
- Encrypted pairing handshake.
- Video encode/decode loopback.
- macOS SCStream-to-VideoToolbox encode loopback.
- macOS Keychain device identity persistence.
- Transport reconnect under packet loss.
- Outbound QoS queue backpressure behavior with a saturated UDP send buffer.
