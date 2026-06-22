# Beta Release Checklist

Before beta:

- [x] Generate Android APK/AAB, Windows MSI, macOS app/pkg, and SHA-256 checksums in one release-candidate workflow.
- [x] Keep release signing material in CI secrets and block unsigned tagged releases.
- [x] Use `VERSION` as the canonical release version and reject Cargo mirror drift.

- [x] Replace Windows GDI capture fallback with DXGI Desktop Duplication.
- [ ] Validate Desktop Duplication continuous capture and access-loss recovery in an interactive Windows session, including lock/unlock and display changes.
- [x] Add a real Windows Media Foundation H.264 software-fallback backend and diagnostic CLI.
- [ ] Add and validate Intel Quick Sync, NVIDIA NVENC, or AMD AMF hardware selection.
- [x] Complete Android LAN receive loop into `RemoteVideoStreamController`.
- [x] Encrypt Windows/Android control, video, and realtime input with signed P-256 ECDH and directional AES-256-GCM.
- [x] Reject post-handshake plaintext/replay packets and enforce trusted-device input permissions before injection.
- [ ] Validate host-identity pin changes, replay windows, expiry, reconnect, and packet reordering on physical devices.
- [ ] Extend the same encrypted session protocol to the macOS host.
- [ ] Validate native pen injection in Krita, OneNote, Clip Studio Paint, Photoshop, and Blender Grease Pencil where possible.
- [ ] Validate Windows DPAPI-backed secret store migration, corrupted-store recovery, and backup/restore behavior.
- [ ] Validate macOS Keychain trusted-client persistence, signed Android `AuthChallenge` / `AuthResponse`, corrupted-store recovery, and replay/expiry failures.
- [ ] Complete macOS first-run permission onboarding.
- [ ] Provision production certificates, run the signing/notarization gate, and define an update strategy.
- [ ] Add crash-safe logging that redacts keyboard and secret material.
- [ ] Run latency benchmarks on a real LAN.
- [ ] Verify Android Samsung S Pen pressure, tilt, hover, eraser, and button behavior.
