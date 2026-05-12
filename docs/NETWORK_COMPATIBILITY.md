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
