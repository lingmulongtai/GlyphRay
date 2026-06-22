# Release And Distribution

GlyphRay can now produce repeatable engineering release candidates. A public production release still requires the signing credentials and hardware validation listed in the release runbook.

## Shared Versioning

- `VERSION` is the canonical `major.minor.patch` release version.
- Cargo workspace packages inherit the Cargo workspace version mirror; release CI rejects a mismatch with `VERSION`.
- Android reads `VERSION` by default and accepts `GLYPHRAY_VERSION_NAME` / `GLYPHRAY_VERSION_CODE` overrides in CI.
- Windows and macOS packaging scripts read `VERSION` unless an explicit version is supplied.
- A release tag must be exactly `v<VERSION>`.

## Windows

Target: Windows 10/11 x64 MSI.

Implemented:

- Release host binary staging under `dist/windows/payload`.
- WiX v4 MSI with a stable upgrade identity, Programs and Features metadata, and Start menu shortcut.
- Optional Authenticode signing of both the host EXE and MSI.
- Local MSI generation verified on Windows with WiX 4.0.6.

Build:

```powershell
powershell -ExecutionPolicy Bypass -File tools/packaging/windows/build-msi.ps1
```

Production gaps: certificate provisioning, installer UI/permission copy, service/agent split, firewall lifecycle, and update delivery.

## macOS

Target: macOS 13+ Developer ID signed and notarized app bundle plus installer package.

Implemented:

- Native `.app` layout with `Info.plist`, minimum OS, local-network, and microphone usage descriptions.
- Release SwiftPM build copied into the app bundle.
- Ad-hoc signing for package smoke tests.
- Optional Developer ID Application and Installer signing.
- Optional `notarytool` submission and stapling for the pkg.
- `.pkg` and zipped app outputs under `dist/macos`.

Build on macOS:

```bash
bash tools/packaging/macos/build-pkg.sh
```

Production gaps: real certificate/notary validation in the repository owner account, first-run onboarding polish, and Sparkle or another signed update strategy.

## Android / Play Store

Target: Play Store internal testing, then closed beta.

Implemented:

- Release APK and AAB Gradle tasks validated locally.
- Version name/code overrides for CI.
- Signing enabled only when all keystore variables are supplied.
- Release workflow accepts the keystore only as an encoded GitHub secret.

Production gaps: Play App Signing enrollment, internal-track upload, privacy policy, Data Safety form, screenshots/store listing, adaptive icon review, and foreground-service policy review.

## Publication Safety

Manual `Release Candidate` workflow runs may be unsigned and are artifact-only. Tag runs refuse to create a GitHub Release unless Android, Windows, and macOS signing are enabled and macOS notarization credentials are present. All candidates include a SHA-256 manifest.
