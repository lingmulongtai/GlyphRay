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

## Message Families

- Handshake: `ClientHello`, `HostHello`
- Authentication: `AuthChallenge`, `AuthResponse`
- Pairing: `PairingRequest`, `PairingResult`
- Display and encoder: `DisplayInfo`, `EncoderConfig`
- Media: `VideoFrame`, `AudioFrame`
- Input: `StylusInputBatch`, `MouseInput`, `KeyboardInput`
- Control: `LatencyPing`, `LatencyPong`, `ErrorMessage`, `Disconnect`
- Optional later: `ClipboardMessage`

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
