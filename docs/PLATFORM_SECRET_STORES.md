# Platform Secret Stores

GlyphRay secret storage boundary:

- Android: `AndroidDeviceKeys` creates a device identity key in Android Keystore.
- Windows: `PlatformSecretStore` is currently an in-process development implementation. It must be replaced with DPAPI or Credential Manager before beta.
- macOS: Keychain integration is required before beta.

Long-term secrets must never be written to logs or checked into the repository.

