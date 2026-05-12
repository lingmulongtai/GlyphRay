# Network Compatibility

GlyphRay is LAN-first, but the transport is plain UDP over IP once the host endpoint is known. That means overlay networks such as Tailscale can work for control and input traffic.

## Tailscale

Current status:

- Android LAN broadcast discovery is not expected to cross Tailscale.
- Android can manually add a host by Tailscale IP or MagicDNS name from the host list screen.
- The Windows host still listens on the same ports: discovery `44998`, control/input `44999`, and video `45000`.

Validation checklist:

- Confirm both devices are on the same tailnet.
- Confirm Windows firewall allows GlyphRay host traffic on the Tailscale interface.
- Add the host manually in Android using the Tailscale IP or MagicDNS name.
- Pair and send latency ping before enabling input injection.

Risks to validate:

- MTU and fragmentation for future video traffic.
- DERP relay latency if a direct Tailscale path is unavailable.
- Firewall profile differences between LAN and Tailscale interfaces.

## Other Overlay VPNs

WireGuard-style overlays should behave similarly if UDP is allowed. Broadcast discovery should be treated as LAN-only unless the overlay explicitly forwards broadcast or multicast traffic.

## UDP Ordering And Backpressure

GlyphRay treats input as real-time state, not reliable history. The host router records the latest accepted input sequence and timestamp for each approved session. Older stylus, touch, mouse, keyboard, or gamepad packets are dropped before injection so packet reordering does not move the pen or pointer backward.

Control responses are queued and flushed with nonblocking UDP sends. This is a short-term guard for pairing/display/latency messages; a dedicated send worker or event loop with backpressure metrics is still planned for sustained video and relay traffic.
