# Packaging

GlyphRay packaging is prepared but not production-ready.

## Windows

- Package target: Windows 10/11 host.
- Candidate tooling: WiX Toolset.
- Service/system tray split should be finalized before beta.
- Host installer must explain input-injection permissions clearly.

## macOS

- Package target: macOS 13+ host.
- Candidate tooling: Xcode archive or signed `.pkg`.
- Required permissions: screen recording, input monitoring/accessibility, microphone/system audio if audio capture is enabled.

## Android

- Package target: debug APK now, Play/internal testing later.
- Signing keys must never be committed.

