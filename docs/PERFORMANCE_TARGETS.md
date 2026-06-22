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

## Latest Local Encoder Probe

On 2026-06-22, `glyphray-encoder-diagnostics` encoded a synthetic 1280x720 BGRA frame through the Microsoft Media Foundation H.264 software MFT into a 5,563-byte Annex B keyframe in 4.114 ms on the latest run. It then packetized the access unit into five GLYT UDP datagrams, with a largest datagram of 1,253 bytes, decoded every datagram, and reconstructed an exact CRC32-matching access unit. The earlier encoder-only run measured 2.576 ms, so both values remain probes rather than a stable benchmark distribution.

DXGI enumeration reported the two active displays at 2560x1440/165 Hz and 2560x1440/120 Hz. `DuplicateOutput` was denied to the current Codex automation desktop with `0x80070005`, so capture latency and full glass-to-glass latency still require an interactive-session run.
