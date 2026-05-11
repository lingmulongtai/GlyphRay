# Performance Targets

## Initial Targets

| Metric | Target |
| --- | ---: |
| LAN 1080p60 glass-to-glass latency | under 35 ms p95 |
| LAN stylus packet arrival to host injection | under 8 ms p95 |
| Frame rate | stable 60 fps |
| Reconnect | fast enough to feel continuous on transient LAN loss |
| Input priority | stylus packets ahead of video under congestion |

## Measurements

GlyphRay should measure:

- encode time
- network time
- decode time
- render time
- input capture time
- input transport time
- input injection time
- estimated end-to-end latency
- CPU/GPU usage in host diagnostics

`crates/telemetry` currently provides local-only latency primitives. It does not send data to external servers.

