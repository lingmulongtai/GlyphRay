# Platform Secret Stores

GlyphRay secret storage boundary:

- Android: `AndroidDeviceKeys` creates a device identity key in Android Keystore.
- Windows: `PlatformSecretStore` stores per-device secrets as atomically replaced DPAPI-protected per-user files under the local GlyphRay app-data directory. Unreadable identity state is moved to a uniquely named quarantine file before a new identity is generated, with an explicit re-pairing warning. CI/non-Windows builds keep an in-memory fallback.
- macOS: `KeychainSecretStore` stores the host P-256 identity and trusted-client records as generic-password items through the Security framework. Updates use `SecItemUpdate` rather than delete-then-add, and corrupt records are copied to unique recovery accounts before regeneration/reset.

Long-term secrets must never be written to logs or checked into the repository.
