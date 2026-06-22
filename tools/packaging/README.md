# Packaging

GlyphRay packaging produces reproducible release candidates. Production distribution still requires signing credentials, macOS notarization, and hardware validation.

## Windows

- Package target: Windows 10/11 host.
- Candidate tooling: WiX Toolset v4.
- Build command: `powershell -ExecutionPolicy Bypass -File tools/packaging/windows/build-msi.ps1`.
- Output target: `dist/windows/GlyphRayHost-<VERSION>.msi`.
- Set `WINDOWS_SIGNING_CERTIFICATE` and `WINDOWS_SIGNING_CERTIFICATE_PASSWORD` to Authenticode-sign the staged EXE and MSI.
- Service/system tray split should be finalized before beta.
- Host installer must explain input-injection permissions clearly.

## macOS

- Package target: macOS 13+ host.
- Candidate tooling: Xcode archive or signed `.pkg`.
- Build command: `bash tools/packaging/macos/build-pkg.sh`.
- Outputs: a macOS `.app`, installer `.pkg`, and zipped app bundle under `dist/macos`.
- Developer ID signing and notarization are enabled only through environment variables; ad-hoc signing is used for smoke builds.
- Required permissions: screen recording, input monitoring/accessibility, microphone/system audio if audio capture is enabled.

## Android

- Package target: debug APK now, Play/internal testing later.
- Release target: Play Store internal testing first, then closed beta.
- Signing keys must never be committed.
- `GLYPHRAY_VERSION_NAME` and `GLYPHRAY_VERSION_CODE` can override CI release metadata.
- Play signing is enabled only when all four `GLYPHRAY_ANDROID_*` signing variables are supplied.
- Required before Play upload: privacy policy, data safety form, store assets, and Android foreground-service policy review.
