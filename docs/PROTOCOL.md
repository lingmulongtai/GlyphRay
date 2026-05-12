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

## Message Families

- Handshake: `ClientHello`, `HostHello`
- Authentication: `AuthChallenge`, `AuthResponse`
- Pairing: `PairingRequest`, `PairingResult`
- Display and encoder: `DisplayInfo`, `EncoderConfig`
- Media: `VideoFrame`, `AudioFrame`
- Input: `StylusInputBatch`, `MouseInput`, `KeyboardInput`
- Control: `LatencyPing`, `LatencyPong`, `ErrorMessage`, `Disconnect`
- Optional later: `ClipboardMessage`

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

The Windows host currently stores approved-client requests. The live capture/encode loop still needs to consume this stored config.

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
