# Backend

The Windows host backend now has the pieces needed for a LAN-first host runtime:

- LAN discovery advertisement (`GLYD`) in `crates/transport/src/discovery.rs`.
- Server-side UDP receive/send in `UdpServer`.
- Host session registry with pending/approved/rejected peers.
- Pairing request routing.
- Permission gating before input packets are accepted.
- Compact stylus packet decode (`GLYS`) and routing into `StylusInputBridge`.
- Latency ping/pong routing.
- `glyphray-windows-host serve` entry point for the host backend loop.

## Running

```powershell
cargo run -p glyphray-windows-host -- serve
```

The backend binds:

- discovery: `44998`
- control/input: `44999`
- video: `45000`

## Current Limits

- The backend loop is still console-driven.
- Peer approval is exposed in code but not yet wired to a host UI prompt.
- Video streaming pipeline exists, but the live control loop is not yet driving capture/encode/send continuously.
- Production Windows secret storage still needs DPAPI or Credential Manager.
- Real app validation for Windows Ink input is still required.

