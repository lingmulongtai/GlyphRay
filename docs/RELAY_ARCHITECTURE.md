# Relay Architecture

GlyphRay remains local-network-first. Relay support is optional and not required for MVP.

The relay architecture should support:

- direct LAN candidate first
- STUN reflexive candidate second
- TURN-style relay candidate as fallback
- mutual authentication before media/input data
- encrypted packets end to end
- no relay visibility into plaintext input or video

`crates/transport/src/relay.rs` currently contains route candidate selection logic. It deliberately prefers trusted direct LAN paths over relay paths.

Relay servers are not implemented in this repository yet.

