# Packaging

GlyphRay packaging is prepared but not production-ready.

## Windows

- Package target: Windows 10/11 host.
- Candidate tooling: WiX Toolset v4.
- Build command: `powershell -ExecutionPolicy Bypass -File tools/packaging/windows/build-msi.ps1`.
- Output target: `dist/windows/GlyphRayHost-0.1.0.msi`.
- Service/system tray split should be finalized before beta.
- Host installer must explain input-injection permissions clearly.

## macOS

- Package target: macOS 13+ host.
- Candidate tooling: Xcode archive or signed `.pkg`.
- Build command: `bash tools/packaging/macos/build-pkg.sh`.
- Required permissions: screen recording, input monitoring/accessibility, microphone/system audio if audio capture is enabled.

## Android

- Package target: debug APK now, Play/internal testing later.
- Release target: Play Store internal testing first, then closed beta.
- Signing keys must never be committed.
- Required before Play upload: release signing config, privacy policy, data safety form, Play-compatible app icon/splash assets, versionCode automation, and Android 14/15 foreground-service policy review.
