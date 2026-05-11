# Security

GlyphRay treats remote desktop access and input injection as high-risk capabilities.

## MVP Model

- LAN-first operation.
- No cloud account required.
- No relay server required.
- Pairing uses a one-time code or QR handoff.
- Devices get stable identity keys.
- Hosts store trusted devices.
- Sessions use short-lived tokens.
- Transport must be encrypted before real input or media data is exchanged.

## Secret Storage

- Android: Android Keystore.
- Windows: DPAPI or Credential Manager.
- macOS: Keychain.

The Rust `SecretStore` trait is intentionally abstract so platform storage is implemented at the platform edge.

## Logging Rules

- Never log raw keyboard input by default.
- Never log passwords or typed text.
- Do not log long-term secrets, pairing secrets, private keys, session tokens, or decrypted packets.
- Stylus diagnostics may show or record stylus values only when explicitly enabled by the user.

## Threats

- Unauthorized host access.
- Man-in-the-middle attack during pairing or reconnect.
- Replay of pairing or session messages.
- Stolen or guessed pairing code.
- Malicious LAN device.
- Input injection abuse on the host.
- Host privilege boundary mistakes.
- Sensitive data leakage through logs.
- Clipboard leakage if clipboard sync is added later.

## Current Implementation

Milestone 1 includes pairing code generation, salted pairing-code hashing, challenge-response helpers, session-token type, and pairing rate limiting. Encrypted transport and platform secret stores are tracked in the roadmap.

