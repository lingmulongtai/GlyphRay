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
- With `GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1`, confirm an approved Android Bluetooth keyboard can inject a safe key sequence.
- Confirm Win and PrintScreen overlay keys are blocked until the host peer is approved and keyboard injection is explicitly enabled.
- With `GLYPHRAY_ENABLE_TOUCH_INJECTION=1`, confirm Android finger input is received by Windows as native touch input in a touch-aware app.
- With `GLYPHRAY_ENABLE_MOUSE_INJECTION=1`, confirm Bluetooth mouse movement/buttons/wheel are injected after approval.
- Confirm gamepad packets are decoded, then validate virtual-controller injection once ViGEm/virtual HID backend exists.

## Integration Tests To Add

- Android-to-host stylus batch replay.
- Android-to-host Bluetooth keyboard replay with Windows virtual-key mapping.
- Android-to-host native touch replay.
- Android-to-host Bluetooth mouse replay.
- Android-to-host gamepad replay.
- Host coordinate mapping under multi-monitor DPI layouts.
- Encrypted pairing handshake.
- Video encode/decode loopback.
- Transport reconnect under packet loss.
