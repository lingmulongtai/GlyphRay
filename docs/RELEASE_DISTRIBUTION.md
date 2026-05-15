# Release And Distribution

GlyphRay is not production-release ready yet, but the repository now tracks the intended release path.

## Windows

Target format: signed installer, preferably MSI or a simple bootstrapper EXE wrapping MSI.

Current foundation:

- WiX v4 package definition in `tools/packaging/windows`.
- Build script: `tools/packaging/windows/build-msi.ps1`.
- Host binary payload staging into `dist/windows/payload`.

Before beta:

- Replace placeholder upgrade GUID with production identity.
- Add code signing.
- Wire the existing `startup status|enable|disable` host commands into installer UI/tray UI, then add the service/agent split.
- Add clear permission copy for pen, touch, mouse, keyboard, and gamepad injection.
- Add uninstall cleanup for services, firewall rules, and trusted device state.

## macOS

Target format: signed and notarized app bundle plus `.pkg`.

Current foundation:

- SwiftPM host executable.
- Permission readiness UI for Screen Recording, Accessibility, Input Monitoring, and audio.
- Keychain secret-store boundary.
- VideoToolbox encoder smoke-test path.
- `pkgbuild` script in `tools/packaging/macos/build-pkg.sh`.

Before beta:

- Convert the host into a proper `.app` bundle.
- Add Developer ID signing and notarization.
- Expand permission readiness into first-run onboarding for Screen Recording, Accessibility, Input Monitoring, and audio permissions.

## Android / Play Store

Target format: Play Store internal testing first, then closed beta.

Before Play upload:

- Add release signing config through local/CI secrets only.
- Add versionCode/versionName automation.
- Add adaptive icon, store icon, screenshots, and privacy policy.
- Complete Play Data Safety form.
- Review Android foreground-service and nearby-network behavior.
- Keep diagnostics opt-in and avoid raw keyboard/input logging.
