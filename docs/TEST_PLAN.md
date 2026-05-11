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
- Transport packet-loss simulation.
- Telemetry p95 and latency totals.
- Windows host encoder settings and start-state behavior.

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

## Windows Manual Tests

- Run `cargo run -p glyphray-pen-diagnostics`.
- Open Krita or OneNote and focus a canvas.
- Confirm a synthetic stroke appears.
- Confirm pressure varies across the stroke.
- Confirm hover/down/up transitions in a pointer inspection tool.

## Integration Tests To Add

- Android-to-host stylus batch replay.
- Host coordinate mapping under multi-monitor DPI layouts.
- Encrypted pairing handshake.
- Video encode/decode loopback.
- Transport reconnect under packet loss.
