# Beta Release Checklist

Before beta:

- [x] Generate Android APK/AAB, Windows MSI, macOS app/pkg, and SHA-256 checksums in one release-candidate workflow.
- [x] Keep release signing material in CI secrets and block unsigned tagged releases.
- [x] Use `VERSION` as the canonical release version and reject Cargo mirror drift.

- [x] Replace Windows GDI capture fallback with DXGI Desktop Duplication.
- [ ] Validate Desktop Duplication continuous capture and access-loss recovery in an interactive Windows session, including lock/unlock and display changes.
- [x] Add a real Windows Media Foundation H.264 software-fallback backend and diagnostic CLI.
- [x] Add hardware MFT selection and validate NVIDIA NVENC through Annex B encode and UDP fragment reassembly.
- [ ] Validate Intel Quick Sync and AMD AMF on representative hardware/driver versions.
- [x] Complete Android LAN receive loop into `RemoteVideoStreamController`.
- [x] Encrypt Windows/Android control, video, and realtime input with signed P-256 ECDH and directional AES-256-GCM.
- [x] Reject post-handshake plaintext/replay packets and enforce trusted-device input permissions before injection.
- [x] Require a salted one-time numeric code before first trust on Windows and macOS; keep signed public-key authentication for returning devices.
- [ ] Validate pairing-code expiry, typo cooldown, and active-LAN attack resistance between physical Android and both desktop hosts.
- [ ] Validate host-identity pin changes, replay windows, expiry, reconnect, and packet reordering on physical devices.
- [x] Extend the signed P-256 ECDH and directional AES-256-GCM session protocol to the macOS host.
- [ ] Validate native pen injection in Krita, OneNote, Clip Studio Paint, Photoshop, and Blender Grease Pencil where possible.
- [x] Add automated Windows DPAPI corrupted-store quarantine/recovery tests.
- [ ] Validate Windows DPAPI migration and backup/restore behavior across installer upgrades.
- [x] Add automated macOS Keychain identity/trust corruption recovery and secure-session replay tests.
- [ ] Validate macOS Keychain persistence, signed Android reconnect, expiry, and recovery on physical devices.
- [ ] Complete macOS first-run permission onboarding.
- [ ] Provision production certificates, run the signing/notarization gate, and define an update strategy.
- [x] Add crash-safe rotating fixed-schema logging that excludes raw keyboard and secret material.
- [ ] Run latency benchmarks on a real LAN.
- [ ] Verify Android Samsung S Pen pressure, tilt, hover, eraser, and button behavior.
