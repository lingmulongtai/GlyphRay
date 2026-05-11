# Backend

The Windows host backend now has the pieces needed for a LAN-first host runtime:

- LAN discovery advertisement (`GLYD`) in `crates/transport/src/discovery.rs`.
- Server-side UDP receive/send in `UdpServer`.
- Host session registry with pending/approved/rejected peers.
- Pairing request routing.
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
- Peer approval is exposed in code but not yet wired to a host UI prompt.
- `GLYPHRAY_DEV_AUTO_APPROVE` is for smoke tests only and should be removed from normal user flows once approval UI exists.
- `GLYPHRAY_ENABLE_PEN_INJECTION` is for explicit native input smoke tests only until display mapping is negotiated.
- Video streaming pipeline exists, but the live control loop is not yet driving capture/encode/send continuously.
- Production Windows secret storage still needs DPAPI or Credential Manager.
- Real app validation for Windows Ink input is still required.
