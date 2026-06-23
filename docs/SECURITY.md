# Security

GlyphRay treats remote desktop access and input injection as high-risk capabilities.

## MVP Model

- LAN-first operation.
- No cloud account required.
- No relay server required.
- First trust uses a salted six-digit one-time code; QR handoff remains optional future UX.
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

First-time pairing on Windows and macOS uses a host-displayed six-digit code that is never transmitted directly. Android receives a peer-specific random salt and returns an HMAC-SHA256 proof under the domain `GlyphRay pairing proof v1`. Challenges expire after two minutes, codes after five minutes, successful proof rotates the code immediately, and five failures in a two-minute window trigger a cooldown. Proofs are stored only in pending session memory and are bound to the requesting endpoint. Permission dialogs and manual approval remain locked until proof verification succeeds; the development auto-approve mode is explicitly opt-in and must not be used for release operation.

Windows, macOS, and Android now implement the same live encrypted session after pairing. A host creates a signed ephemeral P-256 ECDH offer, Android verifies and pins the host identity, and Android returns an ECDSA-signed ephemeral key confirmation using its Keystore identity. Both sides derive separate host-to-client and client-to-host AES-256-GCM keys from the ECDH secret and handshake transcript. Every encrypted `GLYE` datagram has an authenticated monotonic counter; a sliding replay window permits legitimate UDP reordering while rejecting duplicates and stale packets. Rust, Kotlin, and Swift assert the same fixed cross-platform key-derivation vector.

The Windows signing identity is stored in a DPAPI-protected per-user file; the macOS signing identity and trusted-client records are stored in Keychain. Android keeps the device private key in Android Keystore and pins the SHA-256 host identity fingerprint by discovered host id. After secure-session establishment, peers reject plaintext datagrams. Windows and macOS do not emit approved-client video until their secure handshake completes, and Android refuses to emit realtime pen/touch/keyboard/mouse/gamepad input without an active secure codec.

Returning trusted devices complete the ECDSA-signed `AuthChallenge` / `AuthResponse` proof before approval. Windows enforces persisted per-device pen, touch, keyboard, mouse, and gamepad permissions before decoding or injecting input; operators can change them with `trust permission <id> <kind> <on|off>`. macOS currently gates CGEvent mouse, keyboard, and single-touch pointer injection on the encrypted session, but still needs a graphical per-device input permission editor matching the Windows policy surface.

The Windows host router also limits pending unapproved sessions, rate limits new pending attempts per source IP, evicts the oldest pending peer when the global cap is reached, drops late input packets before native injection, queues outbound packets through bounded nonblocking per-channel QoS queues, and exposes local-only health counters for rate limits, queue drops, and backpressure.

Windows DPAPI state and macOS Keychain identity/trust records now have automated corruption quarantine/recovery coverage, and Windows writes settings/secrets through atomic replacement. Beta still needs installer-upgrade migration and physical backup/restore validation, physical-device pairing/replay/expiry attack testing, optional QR presentation, macOS reconnect/backpressure soak validation, and graphical trusted-device permission editors.
