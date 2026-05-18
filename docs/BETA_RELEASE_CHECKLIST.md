# Beta Release Checklist

Before beta:

- Replace Windows GDI capture fallback with Windows Graphics Capture or Desktop Duplication.
- Add real H.264 encoder backend.
- Complete Android LAN receive loop into `RemoteVideoStreamController`.
- Validate native pen injection in Krita, OneNote, Clip Studio Paint, Photoshop, and Blender Grease Pencil where possible.
- Validate Windows DPAPI-backed secret store migration, corrupted-store recovery, and backup/restore behavior.
- Validate macOS Keychain trusted-client persistence, signed Android `AuthChallenge` / `AuthResponse`, corrupted-store recovery, and replay/expiry failures.
- Complete macOS first-run permission onboarding.
- Add installer signing and update strategy.
- Add crash-safe logging that redacts keyboard and secret material.
- Run latency benchmarks on a real LAN.
- Verify Android Samsung S Pen pressure, tilt, hover, eraser, and button behavior.
