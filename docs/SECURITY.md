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

Windows and Android now establish a live encrypted session after pairing. The Windows host creates a signed ephemeral P-256 ECDH offer, Android verifies and pins the host identity, and Android returns an ECDSA-signed ephemeral key confirmation using its Keystore identity. Both sides derive separate host-to-client and client-to-host AES-256-GCM keys from the ECDH secret and handshake transcript. Every encrypted `GLYE` datagram has an authenticated monotonic counter; a sliding replay window permits legitimate UDP reordering while rejecting duplicates and stale packets. Rust and Kotlin assert the same fixed cross-platform key-derivation vectors.

The Windows signing identity is generated once and stored in a DPAPI-protected per-user file. Android keeps the device private key in Android Keystore and pins the SHA-256 host identity fingerprint by discovered host id. After secure-session establishment, both peers reject plaintext datagrams. The Windows runtime does not queue video for a peer until its secure handshake completes, and Android refuses to emit realtime pen/touch/keyboard/mouse/gamepad input without an active secure codec.

Returning trusted devices still complete the ECDSA-signed `AuthChallenge` / `AuthResponse` proof before approval. Windows enforces persisted per-device pen, touch, keyboard, mouse, and gamepad permissions before decoding or injecting input; operators can change them with `trust permission <id> <kind> <on|off>`. The macOS lightweight control runtime has signed returning-client verification and Keychain persistence, but it does not yet implement the `GLYH`/`GLYE` live-session handshake.

The Windows host router also limits pending unapproved sessions, rate limits new pending attempts per source IP, evicts the oldest pending peer when the global cap is reached, drops late input packets before native injection, queues outbound packets through bounded nonblocking per-channel QoS queues, and exposes local-only health counters for rate limits, queue drops, and backpressure.

Beta still needs secret-store migration/corruption recovery tests, macOS live-session encryption, first-pairing out-of-band fingerprint or QR verification, real-device replay/expiry validation, and a graphical trusted-device permission editor.
