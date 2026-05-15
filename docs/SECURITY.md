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
- Memory exhaustion from malicious LAN peers that rotate UDP source ports.
- Connection starvation from a single IP rotating source ports until legitimate pending peers are evicted.
- Host responsiveness loss from synchronous sends blocking the receive path.

## Current Implementation

Current code includes pairing code generation, salted pairing-code hashing, challenge-response helpers, session-token type, pairing rate limiting, a ChaCha20-Poly1305 session cipher, replay protection, and a secure datagram codec foundation. Android device identity keys use Android Keystore and the Android pairing request carries the public key bytes so the Windows host can store a SHA-256 trusted-device fingerprint and DER public key. Returning trusted devices now complete an ECDSA signed `AuthChallenge` / `AuthResponse` proof before the host accepts them. Windows host secrets are protected with DPAPI-backed per-user files, and the macOS host has a Security-framework Keychain secret store boundary.

The Windows host router also limits pending unapproved sessions, rate limits new pending attempts per source IP, evicts the oldest pending peer when the global cap is reached, drops late input packets before native injection, queues outbound packets through bounded nonblocking per-channel QoS queues, and exposes local-only health counters for rate limits, queue drops, and backpressure.

macOS Keychain now exists as a code-level store, but still needs device identity wiring and migration tests. Windows DPAPI storage exists, returning Android devices can prove ownership of the saved public key, and beta still needs migration, backup, corrupted-store recovery tests, and UI review of trusted-device permissions.
