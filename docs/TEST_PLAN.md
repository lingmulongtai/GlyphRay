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
- Confirm an authenticated Android Bluetooth keyboard can inject a safe key sequence, then set keyboard permission off and verify injection stops immediately.
- Confirm Win and PrintScreen overlay keys are blocked until the host peer is approved and keyboard injection is explicitly enabled.
- Confirm Android finger input is received by Windows as native touch input only after approval and secure-session establishment.
- Confirm Bluetooth mouse movement/buttons/wheel are injected after approval, and rejected when mouse permission is off.
- With `GLYPHRAY_ENABLE_PERMISSION_DIALOG=1`, send a first-time pairing request from Android, confirm Android receives `PairingChallenge`, enter the six-digit code printed by Windows, then approve in the Windows dialog and confirm `PairingResult accepted=true` plus encrypted `DisplayInfo`.
- Enter a wrong code five times and confirm further challenges are blocked until the two-minute attempt window resets. Confirm a challenge expires after two minutes, the displayed code rotates after five minutes or immediately after success, and a captured proof cannot be reused from another UDP endpoint.
- After approving a device, run `trust list` and confirm the trusted-device id, label, last peer, approval timestamp, and permission flags are present. Run `trust forget <id>` and confirm only that record is removed; run `trust clear` on a disposable profile and confirm all records are removed.
- Pair the same Android device twice and confirm the second request receives `AuthChallenge`, Android replies with `AuthResponse`, and the host accepts only when the saved Android public-key fingerprint and ECDSA signature verify. Delete the trusted record and confirm approval is required again.
- Confirm gamepad packets are decoded and reach the Windows virtual gamepad bridge. Once a ViGEm/virtual HID backend is linked, validate that Windows sees an Xbox-compatible controller, buttons/sticks/triggers update correctly, disconnect removes the virtual controller, and no controller input is accepted when the trusted-device gamepad permission is disabled.

## macOS Manual Tests

- Run `swift test -c release` and `swift build -c release` on macOS 13+.
- Confirm GitHub Actions `CI / macOS host SwiftPM build` passes secure-session and Android-compatible input wire tests on `macos-14`.
- Launch the host and confirm Screen Recording, Accessibility, Input Monitoring, and audio readiness states are visible.
- Use the Screen Recording, Accessibility, and Audio request buttons and confirm macOS opens the expected permission prompts.
- Confirm ScreenCaptureKit display listing shows each available display with geometry.
- Start the macOS Control runtime and confirm the discovery status reports sent announcements. On a LAN that allows broadcast, confirm Android host discovery shows the macOS host.
- Start the macOS Control runtime, add the macOS host manually in Android using UDP port `44999`, send a pairing request, enter the code displayed in the macOS window, and confirm macOS lists the approved client while Android receives `PairingResult`.
- After pairing, confirm the macOS status reaches one secure client, Android receives encrypted `DisplayInfo`, and plaintext latency/input is rejected.
- Press `Start Approved Stream` and confirm it is disabled before key confirmation, then streams encrypted `GLYE` video to the approved endpoint after confirmation.
- Restart the macOS host after a successful pairing and confirm the trusted client list is restored from Keychain. Pair the same Android device again and confirm macOS sends `AuthChallenge`, Android returns `AuthResponse`, and macOS accepts only after signature verification. Use `Clear Trust` and confirm the list is removed.
- Run the Live Capture Probe and confirm a short `SCStream` session reports at least one captured frame.
- Run the VideoToolbox encoder smoke test and confirm a low-latency H.264 session can be created.
- Run the Live Encode Probe and confirm captured frame count, encoded frame count, and encoded byte count are non-zero after Screen Recording permission is granted.
- Run the Live Transport Probe and confirm captured frame count, encoded frame count, Video-channel datagram count, and transport byte count are non-zero.
- Request a non-native resolution, 60/90/120 fps, bitrate, and keyframe interval from Android; confirm the stream reports the selected display and clamped output settings. Confirm unsupported codecs fail explicitly.
- Leave the approved encrypted stream active for at least ten seconds, then stop it and confirm the UI reports non-zero encoded frames, scheduled/sent datagrams, bytes, high watermark, and drops. Under artificial delay, confirm drops increase instead of unbounded latency.
- Run the Keychain smoke test and confirm save/load/delete passes before wiring device identity.
- Run the Windows DPAPI `PlatformSecretStore` round-trip test and confirm secrets survive reopening the store.
- Run `glyphray-windows-host startup status`, then enable and disable startup on a disposable Windows test user and confirm the HKCU Run value changes as expected.
- Run `encoder override 1920x1080 120 35000`, `encoder save`, restart the backend, and confirm `encoder status` reports the saved override before any client connects.
- Run `encoder preset save studio-120`, `encoder preset list`, `encoder preset apply studio-120`, and `encoder preset delete studio-120`; confirm apply restarts the default video pump and delete removes only the named preset, not the saved default override.
- With Accessibility granted, validate encrypted Bluetooth mouse movement/buttons/wheel, keyboard letters/modifiers/function keys, and single-finger touch pointer down/drag/up. Confirm stale input sequences are dropped.

## Integration Tests To Add

- Android-to-host stylus batch replay.
- Android-to-host Bluetooth keyboard replay with Windows virtual-key mapping.
- Android-to-host native touch replay.
- Android-to-host Bluetooth mouse replay.
- Android-to-host gamepad replay.
- Host coordinate mapping under multi-monitor DPI layouts.
- Encrypted pairing handshake.
- Video encode/decode loopback.
- macOS signed trusted-device proof expiry/replay/failure cases plus SCStream-to-VideoToolbox-to-UDP Android client loopback.
- macOS Keychain trusted-client migration and corrupted-store recovery.
- Transport reconnect under packet loss.
- Outbound QoS queue backpressure behavior with a saturated UDP send buffer.
