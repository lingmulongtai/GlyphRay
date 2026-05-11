# Beta Release Checklist

Before beta:

- Replace Windows GDI capture fallback with Windows Graphics Capture or Desktop Duplication.
- Add real H.264 encoder backend.
- Complete Android LAN receive loop into `RemoteVideoStreamController`.
- Validate native pen injection in Krita, OneNote, Clip Studio Paint, Photoshop, and Blender Grease Pencil where possible.
- Replace Windows development secret store with DPAPI or Credential Manager.
- Add macOS Keychain storage.
- Add installer signing and update strategy.
- Add crash-safe logging that redacts keyboard and secret material.
- Run latency benchmarks on a real LAN.
- Verify Android Samsung S Pen pressure, tilt, hover, eraser, and button behavior.

