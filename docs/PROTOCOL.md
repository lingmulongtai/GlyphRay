# Protocol

GlyphRay uses a versioned binary protocol. High-frequency input and media payloads must not use JSON.

## Frame Format

Current frame header size: 24 bytes.

| Field | Size | Description |
| --- | ---: | --- |
| magic | 4 | `GLYR` |
| wire_version | 2 | little-endian, currently `1` |
| message_kind | 2 | little-endian enum discriminator |
| sequence | 8 | transport/session sequence |
| payload_len | 4 | bytes after header |
| crc32 | 4 | payload checksum |

Payloads are currently encoded with `bincode` over Rust schema types in `crates/protocol`. The frame boundary keeps room to move to FlatBuffers, Cap'n Proto, or a hand-rolled stable schema if needed.

## UDP Transport Datagram

`crates/transport` now defines a lightweight UDP datagram wrapper for transport packets:

| Field | Size | Description |
| --- | ---: | --- |
| magic | 4 | `GLYT` |
| datagram_version | 2 | little-endian, currently `1` |
| channel | 1 | video, audio, input, or control |
| message_kind | 2 | protocol message kind |
| sequence | 8 | packet sequence |
| enqueue_timestamp_us | 8 | local monotonic enqueue timestamp |
| payload_len | 4 | bytes after datagram header |
| crc32 | 4 | payload checksum |

The datagram layer is intentionally smaller than the session protocol frame. It is suitable for input/control packets now and will need video frame chunking before large encoded frames are sent over UDP.

The input channel is ordered as real-time state. Receivers should treat older sequence numbers or backward-moving input timestamps as late packets and drop them before OS injection. This avoids visible cursor or pen jitter when UDP packets arrive out of order.

## First-Time Pairing Proof

An untrusted Android device first sends `PairingRequest` with an empty `pairing_code_hash` and its Android Keystore public key. A Windows or macOS host responds with `PairingChallenge` (message kind `23`, bincode message variant `20`) containing a peer-specific 32-byte random salt, an absolute expiry, and `code_digits = 6`. The host displays a freshly generated `DDD-DDD` code locally; the code itself is never sent over the network.

Android canonicalizes the entered code to six ASCII digits and sends a second `PairingRequest` with:

```text
HMAC-SHA256(key = challenge_salt,
            data = "GlyphRay pairing proof v1" || six_ascii_digits)
```

The challenge is bound to the source endpoint and expires after two minutes. The displayed code expires after five minutes and rotates immediately after one successful proof, invalidating other outstanding challenges. Five failed proofs within two minutes temporarily block further attempts. Only a verified request reaches the host permission dialog or `approve` command. Returning devices whose stored public-key fingerprint matches skip the numeric code and complete signed `AuthChallenge` / `AuthResponse` verification instead.

## Secure Session Handshake And Datagram

After an accepted pairing or trusted-device authentication, Windows or macOS sends a `SessionKeyExchange` transport packet with a binary `GLYH` payload. It contains a 16-byte session id, expiry, 32-byte salt, ephemeral P-256 public key DER, persistent host identity public key DER, and host ECDSA signature. Android verifies the offer, checks its pinned host fingerprint, creates an ephemeral P-256 key, and returns a signed `SessionKeyConfirm` `GLYH` payload.

Both peers hash the same transcript and derive independent `host-to-client` and `client-to-host` AES-256-GCM keys. Directional keys prevent nonce reuse when both sides start counters at one. Once confirmation succeeds, each complete `GLYT` packet is sealed inside `GLYE`:

| Field | Size | Description |
| --- | ---: | --- |
| magic | 4 | `GLYE` |
| version | 2 | little-endian, currently `1` |
| counter | 8 | little-endian authenticated replay counter |
| ciphertext_len | 4 | encrypted bytes including the GCM tag |
| ciphertext | variable | AES-256-GCM sealed `GLYT` datagram |

The AES-GCM nonce is the ASCII prefix `GLYR` plus the counter encoded as big-endian `u64`. Associated data is `GlyphRay secure datagram v1` followed by the 16-byte session id. Windows, Android, and macOS reject plaintext after the session becomes secure and use a 4096-counter replay window for reordered UDP delivery.

## Video Fragment Payload

Large encoded frames can be split with the transport-level `GLYF` fragment payload:

| Field | Size | Description |
| --- | ---: | --- |
| magic | 4 | `GLYF` |
| frame_sequence | 8 | encoded frame sequence |
| fragment_index | 2 | zero-based fragment index |
| fragment_count | 2 | fragments in the full frame |
| payload_len | 4 | fragment payload length |

`FrameReassembler` rebuilds the encoded frame once every fragment for a sequence has arrived. Loss recovery and retransmission policy are still part of the Milestone 2 streaming work.

The complete encoded access unit inside the fragment stream is:

| Field | Size | Description |
| --- | ---: | --- |
| codec | 1 | H.264, H.265, or AV1 |
| is_keyframe | 1 | `0` or `1` |
| sequence | 8 | encoded frame sequence |
| presentation_time_us | 8 | presentation timestamp |
| payload_len | 4 | encoded frame payload length |
| payload | variable | codec bytes, currently H.264 Annex B expected by Android decoder |

Android mirrors this reassembly path in `VideoFragmentReassembler.kt`.

The macOS host mirrors the sender side in `MacVideoTransportPacketizer.swift`: `MacEncodedFrame` payloads are wrapped into the same encoded access-unit envelope, split into `GLYF` fragments, and then wrapped in `GLYT` Video-channel datagrams. The VideoToolbox H.264 path converts length-prefixed NAL units into Annex B and prepends SPS/PPS on keyframes. `MacUdpVideoPublisher.swift` accepts only an explicit datagram transform; the production caller supplies the approved client's `GLYE` codec, so the video publisher has no identity plaintext fallback. Its bounded in-flight queue drops new video datagrams instead of accumulating latency.

`MacControlRuntime.swift` implements the shared signed `GLYH` handshake and directional `GLYE` codec with a 4096-counter replay window. The persistent host signing identity and trusted Android public keys are stored through Keychain. After key confirmation it sends encrypted `DisplayInfo`, requires encryption for latency and realtime input, seals approved-client video, and chooses only active secure targets for streaming. It also applies client `EncoderConfig` display, resolution, FPS, bitrate, and keyframe settings. macOS CI, real Android interoperability, and long-run reconnect/backpressure behavior still need validation.

`MacLanDiscoveryAdvertiser.swift` emits `GLYD` discovery advertisements using the same host advertisement shape as the Rust transport crate. It marks H.264 support and pairing-required mode, but not Windows Ink support because native Windows Ink-style pen injection remains Windows-specific.

`MacTrustedClientStore.swift` persists approved macOS client metadata through Keychain so the host can restore the most recent Android client endpoint on launch. Stored records include endpoint metadata, SHA-256 public-key fingerprint, and public-key DER when Android provides Keystore identity material. Returning clients with a matching fingerprint are challenged and must return a valid ECDSA `AuthResponse` before approval.

## Message Families

- Handshake: `ClientHello`, `HostHello`
- Authentication: `AuthChallenge`, `AuthResponse`
- Pairing: `PairingRequest`, `PairingChallenge`, `PairingResult`
- Display and encoder: `DisplayInfo`, `EncoderConfig`
- Media: `VideoFrame`, `AudioFrame`
- Input: `StylusInputBatch`, `TouchInputBatch`, `MouseInput`, `KeyboardInput`, `GamepadInput`
- Control: `LatencyPing`, `LatencyPong`, `ErrorMessage`, `Disconnect`
- Optional later: `ClipboardMessage`

## Trusted Device Authentication

After a manual approval, the Windows host and macOS host store the Android Keystore public key DER and its SHA-256 fingerprint in trusted-device state. On a later pairing request with the same fingerprint, the host sends `AuthChallenge` instead of approving immediately.

The Android client signs this stable payload with `SHA256withECDSA` over its Keystore EC P-256 private key:

| Field | Encoding |
| --- | --- |
| domain | ASCII bytes `GlyphRay trusted device challenge v1` |
| challenge id | little-endian `u64` |
| nonce | 32 raw bytes |
| trusted device id length | little-endian `u64` |
| trusted device id | UTF-8 bytes |

The client returns `AuthResponse { challenge_id, device_id, signature }`. The host verifies the DER ECDSA signature with the saved public key before sending an accepted `PairingResult`.

## EncoderConfig

`EncoderConfig` is the bidirectional video preference message used by the client now and by host-side UI overrides later:

- display id
- codec: H.264, H.265, or AV1
- color space: sRGB, Display P3, Rec.709, or Rec.2020 PQ
- width and height
- max fps
- target bitrate kbps
- keyframe interval
- low-latency mode flag

The Windows host stores approved-client requests and the opt-in live capture/encode loop consumes them for FPS, bitrate, codec, color space, and keyframe settings. Resolution scaling is still pending, so the host may keep capture-native dimensions until the scaler lands.

## StylusInputBatch

`StylusInputBatch` carries many samples in one small packet:

- batch sequence
- monotonic timestamp
- sample sequence
- sample timestamp
- display id
- pointer id
- tool type
- action
- x/y
- pressure
- tilt X/Y
- orientation
- button flags
- hover flag
- eraser flag
- predicted flag

Input channel packets must be prioritized over video when congestion appears.

## KeyboardInput

`KeyboardInput` carries key state without raw typed text:

- sequence
- timestamp
- hardware scan code when available
- Windows virtual key
- pressed flag
- modifier bitfield

The Android client maps common `KeyEvent` codes to Windows virtual keys before transmission. The Windows host can decode these packets and, when explicitly enabled for smoke testing, inject them through `SendInput`. Layout-aware text input and IME behavior remain future work.

## TouchInputBatch

`TouchInputBatch` exists so Android finger input can become native Windows `PT_TOUCH` input rather than stylus or mouse emulation:

- batch sequence
- monotonic timestamp
- display id
- sample sequence and timestamp
- pointer id
- action: down, move, up, cancel
- x/y
- pressure
- major/minor contact size
- orientation
- flags

The Windows host injects approved touch packets through `PT_TOUCH` after encrypted-session and per-device touch-permission checks. Selected-display mapping is wired; multi-monitor rotation, calibration, and real-device gesture validation still require hardware testing.

## MouseInput

`MouseInput` carries Bluetooth/USB mouse motion from the Android client:

- sequence
- timestamp
- display id
- x/y
- horizontal and vertical wheel deltas
- button flags

The Windows host can inject cursor movement, primary/secondary/middle buttons, and wheel events after encrypted-session and per-device mouse-permission checks.

The macOS host decodes the same encrypted mouse and keyboard payloads and posts mapped CGEvents after its secure session is established. Single-finger `TouchInputBatch` is currently translated to pointer down/drag/up because public macOS APIs do not provide Windows-style native synthetic multi-touch injection.

## GamepadInput

`GamepadInput` carries Android-connected controller state:

- sequence
- timestamp
- controller id
- connected flag
- button bitset
- left/right triggers
- left/right stick axes

The Windows host decodes these reports, checks the trusted-device gamepad permission before processing them, and routes approved packets through a virtual gamepad bridge that clamps triggers/sticks and prepares an XInput-style report. Actual Windows controller presentation still needs a linked ViGEm or signed virtual HID native backend plus hardware validation.

## Compact Stylus Wire Packet

Android and Windows also share a compact stylus packet format for high-frequency input transport. It starts with `GLYS`, version `1`, batch sequence, monotonic timestamp, sample count, and reserved bytes. Each sample is 58 bytes:

| Field | Size |
| --- | ---: |
| sequence | 8 |
| timestamp_us | 8 |
| display_id | 4 |
| pointer_id | 4 |
| tool_type | 1 |
| action | 1 |
| x | 4 |
| y | 4 |
| pressure | 4 |
| tilt_x_degrees | 4 |
| tilt_y_degrees | 4 |
| orientation_degrees | 4 |
| button_flags | 4 |
| flags | 1 |
| reserved | 3 |

Flags: bit 0 hover, bit 1 eraser, bit 2 predicted.
