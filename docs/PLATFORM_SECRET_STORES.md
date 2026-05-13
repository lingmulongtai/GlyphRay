# Platform Secret Stores

GlyphRay secret storage boundary:

- Android: `AndroidDeviceKeys` creates a device identity key in Android Keystore.
- Windows: `PlatformSecretStore` stores per-device secrets as DPAPI-protected per-user files under the local GlyphRay app-data directory. CI/non-Windows builds keep an in-memory fallback.
- macOS: `KeychainSecretStore` stores generic-password secrets through the Security framework. It still needs device identity wiring and migration tests before beta.

Long-term secrets must never be written to logs or checked into the repository.
