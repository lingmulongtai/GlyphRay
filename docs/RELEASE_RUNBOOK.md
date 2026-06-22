# Release Runbook

This runbook is the authoritative path from a clean commit to a public GlyphRay release.

## 1. Preflight

1. Update `VERSION` and `[workspace.package].version` in `Cargo.toml` to the same `major.minor.patch` value.
2. Update both READMEs, the roadmap, and the development diary.
3. Run the local gates:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
.\gradlew.bat :apps:android-client:testDebugUnitTest :apps:android-client:lintRelease :apps:android-client:assembleRelease :apps:android-client:bundleRelease
powershell -ExecutionPolicy Bypass -File tools/packaging/windows/build-msi.ps1
```

4. Confirm `CI` and `Package Smoke Checks` are green, including the macOS runner.
5. Complete every unchecked item in `docs/BETA_RELEASE_CHECKLIST.md` required for the intended channel.

## 2. GitHub Secrets

Android:

- `ANDROID_RELEASE_KEYSTORE_BASE64`
- `ANDROID_RELEASE_STORE_PASSWORD`
- `ANDROID_RELEASE_KEY_ALIAS`
- `ANDROID_RELEASE_KEY_PASSWORD`

Windows:

- `WINDOWS_SIGNING_CERTIFICATE_BASE64`
- `WINDOWS_SIGNING_CERTIFICATE_PASSWORD`

macOS:

- `MACOS_SIGNING_CERTIFICATE_BASE64`
- `MACOS_SIGNING_CERTIFICATE_PASSWORD`
- `MACOS_APP_SIGNING_IDENTITY`
- `MACOS_INSTALLER_SIGNING_IDENTITY`
- `MACOS_NOTARY_APPLE_ID`
- `MACOS_NOTARY_TEAM_ID`
- `MACOS_NOTARY_PASSWORD`

Encode binary certificate files as one-line Base64 values. Never commit certificate files or passwords.

## 3. Engineering Candidate

Run the `Release Candidate` workflow manually. This produces Android APK/AAB, Windows MSI, macOS pkg/zip, and `SHA256SUMS.txt`. Manual candidates may be unsigned and must not be presented as production releases.

Install the candidate on clean test devices and run:

- Pairing, reconnect, trust revoke, and corrupted-state recovery.
- 30-minute 1080p60 LAN session with packet loss introduced during the run.
- S Pen pressure, tilt, hover, eraser, and barrel button checks.
- Keyboard, touch, Bluetooth mouse, and controller checks.
- Windows uninstall/reinstall and macOS permission reset/regrant checks.

Record the device, OS build, network topology, and measured p50/p95 latency in the release notes.

## 4. Production Tag

Create and push only the tag matching `VERSION`:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The workflow will stop before publication if any platform signing path or macOS notarization is absent. When all gates pass, it creates the GitHub Release with checksums.

## 5. Store And Post-Release

- Upload the signed AAB to Play internal testing before broader rollout.
- Verify Authenticode on the downloaded EXE/MSI and Gatekeeper/notarization on the downloaded macOS pkg.
- Verify hashes against `SHA256SUMS.txt` from a separate download.
- Keep the previous release available for rollback.
- Do not enable automatic update delivery until signed update metadata and rollback behavior are tested.
