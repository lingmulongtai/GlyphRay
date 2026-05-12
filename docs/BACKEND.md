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
cargo run -p glyphray-windows-host -- serve
```

`GLYPHRAY_DEV_AUTO_APPROVE` bypasses the approval UI for local smoke tests. `GLYPHRAY_ENABLE_PEN_INJECTION` connects the backend router to the native Win32 synthetic pen injector when that API is available. Both switches are intentionally explicit and must not become the production permission model.

Without development auto-approval, the console loop prints incoming pairing requests. Use:

```powershell
sessions
approve 192.168.1.20:44999
reject 192.168.1.20:44999
```

Approval and rejection both send a protocol-level `PairingResult` back to the Android control channel. Accepted pairing also queues `DisplayInfo` so the client can see available monitor geometry.

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
- Production Windows secret storage still needs DPAPI or Credential Manager.
- Real app validation for Windows Ink input is still required.
