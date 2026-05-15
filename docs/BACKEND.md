# Backend

The Windows host backend now has the pieces needed for a LAN-first host runtime:

- LAN discovery advertisement (`GLYD`) in `crates/transport/src/discovery.rs`.
- Server-side UDP receive/send in `UdpServer`.
- Host session registry with pending/approved/rejected peers.
- Pairing request routing.
- Console approval/rejection commands that send `PairingResult`.
- Host monitor `DisplayInfo` response after accepted pairing.
- Client `EncoderConfig` intake for resolution, refresh rate, bitrate, color space, codec, and low-latency settings.
- Keyboard packet decode and opt-in `SendInput` injection for Bluetooth keyboard and special-key smoke tests.
- Native touch packet decode and opt-in `PT_TOUCH` injection for Android finger input smoke tests.
- Bluetooth mouse packet decode and opt-in native cursor/button/wheel injection.
- Gamepad packet decode for Android-connected controllers.
- Permission gating before input packets are accepted.
- Pending-session cap of 50 unapproved peers, with oldest pending peer eviction to limit UDP spam memory growth.
- Per-IP new pending attempt rate limiting to prevent one host from starving other pending peers by rotating source ports.
- Late input packet dropping based on per-session transport sequence and input timestamp watermarks.
- Bounded nonblocking outbound queues split by channel, with a small QoS schedule that favors input/control over audio/video.
- Backend health snapshots and a console `status` command for session counts, queue depth, drops, late input drops, pending rate limits, and backpressure events.
- Approved-peer video fragment queueing. `GLYPHRAY_ENABLE_VIDEO_STREAM=1` drives the capture/encode/packetize loop and queues `VideoFrame` packets on the Video channel for approved clients.
- Development-only auto-approval mode for local LAN input-path smoke tests.
- Compact stylus packet decode (`GLYS`) and routing into `StylusInputBridge`.
- Latency ping/pong routing.
- `glyphray-windows-host serve` entry point for the host backend loop.

## Running

```powershell
cargo run -p glyphray-windows-host -- serve
```

For early LAN input testing before the host approval UI exists:

```powershell
$env:GLYPHRAY_DEV_AUTO_APPROVE='1'
$env:GLYPHRAY_ENABLE_PEN_INJECTION='1'
$env:GLYPHRAY_ENABLE_VIDEO_STREAM='1'
cargo run -p glyphray-windows-host -- serve
```

`GLYPHRAY_DEV_AUTO_APPROVE` bypasses the approval UI for local smoke tests. `GLYPHRAY_ENABLE_PEN_INJECTION` connects the backend router to the native Win32 synthetic pen injector when that API is available. Both switches are intentionally explicit and must not become the production permission model.

Without development auto-approval, the console loop prints incoming pairing requests. Use:

```powershell
sessions
status
approve 192.168.1.20:44999
reject 192.168.1.20:44999
```

Approval and rejection both send a protocol-level `PairingResult` back to the Android control channel. Accepted pairing also queues `DisplayInfo` so the client can see available monitor geometry.

The backend drops out-of-order input packets after approval if the incoming transport sequence is not newer than the last accepted input packet, or if the input timestamp moves backward. This intentionally prefers a stable current pen/cursor position over replaying late UDP arrivals.

Control responses are queued into bounded in-memory queues and flushed with nonblocking UDP sends. The queues are split by `ChannelKind`, and the current QoS schedule favors input and control packets over audio/video so future media backlog cannot delay pairing, latency, or input-critical control traffic. If the OS send buffer is temporarily full, the receive path keeps ownership of the packet and retries on a later poll instead of blocking the whole control loop. A dedicated send worker or event loop is still planned before beta.

Pending eviction currently scans pending sessions to find the oldest entry. This is intentionally simple because the host cap is 50. If the same logic is reused for relay-scale workloads, replace it with an indexed queue, timer heap, or equivalent O(log N)/O(1) structure.

`status` prints a compact local health snapshot. The most useful early fields are outbound channel depths, queue high watermark, dropped outbound packets, late input drops, and pending rate-limited packets. This is intentionally local-only console output; it does not send telemetry to external services.

`encoder save` persists the active host override, or the latest approved client `EncoderConfig`, to the local host settings file. The backend reloads that override on startup. `encoder clear` clears both the active override and the saved override.

`startup status`, `startup enable`, and `startup disable` manage the current user's Windows startup registration. The implementation uses the HKCU Run key and launches the host with `serve` after user logon.

The video pump currently uses the existing capture/encoder abstraction and queues fragmented encoded access units to approved peers. The queueing path is real, but the default `PendingHardwareEncoder` is still a placeholder; production video still needs a concrete H.264 hardware/software encoder backend.

The current opt-in pen injection bridge uses temporary 1920x1080 stretch mapping. Display negotiation, selected monitor geometry, high-DPI scaling, and calibration must replace this before beta use.

The backend binds:

- discovery: `44998`
- control/input: `44999`
- video: `45000`

Android now has matching `GLYD` discovery decode and `GLYT` stylus datagram encode foundations in:

- `apps/android-client/src/main/java/com/glyphray/android/network/HostDiscovery.kt`
- `apps/android-client/src/main/java/com/glyphray/android/network/TransportPacketCodec.kt`

## Current Limits

- The backend loop is still console-driven.
- The outbound control queue is a short-term nonblocking guard, not a full transport scheduler.
- Per-IP rate limits are in-memory only and are visible through console `status`; they still need richer diagnostics UI before beta.
- Peer approval is console-driven and still needs a native host UI prompt.
- DisplayInfo uses current monitor enumeration and should later feed selected-monitor mapping and calibration.
- Client encoder config is stored on the session but is not yet wired into the live capture/encode loop.
- Keyboard packets can be injected with native Windows `SendInput` when `GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1` is explicitly set.
- Keyboard injection currently uses Windows virtual keys and needs layout-aware text/IME handling before beta.
- `GLYPHRAY_DEV_AUTO_APPROVE` is for smoke tests only and should be removed from normal user flows once approval UI exists.
- `GLYPHRAY_ENABLE_PEN_INJECTION` is for explicit native input smoke tests only until display mapping is negotiated.
- `GLYPHRAY_ENABLE_KEYBOARD_INJECTION` is for explicit keyboard smoke tests only until the host permission UI exists.
- `GLYPHRAY_ENABLE_TOUCH_INJECTION` is for explicit native touch smoke tests only until monitor mapping/calibration is negotiated.
- `GLYPHRAY_ENABLE_MOUSE_INJECTION` is for explicit mouse smoke tests only until the host permission UI exists.
- Gamepad reports are decoded, but Windows virtual-controller injection needs a ViGEm or virtual HID backend.
- Video streaming pipeline exists, but the live control loop is not yet driving capture/encode/send continuously.
- Windows platform secret storage uses DPAPI-protected per-user files. Beta still needs migration and corrupted-store recovery tests.
- Real app validation for Windows Ink input is still required.
