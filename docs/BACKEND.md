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
cargo run -p glyphray-windows-host -- serve
```

This bypass is intentionally explicit and must not become the production permission model.

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
- Video streaming pipeline exists, but the live control loop is not yet driving capture/encode/send continuously.
- Production Windows secret storage still needs DPAPI or Credential Manager.
- Real app validation for Windows Ink input is still required.
