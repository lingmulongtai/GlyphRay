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

On 2026-06-23, `glyphray-encoder-diagnostics` enumerated two AMD H.264 MFT registrations and one NVIDIA H.264 Encoder MFT, skipped candidates that could not configure, and selected NVENC in Auto mode. An optimized build converted a synthetic 1280x720 BGRA frame into an 8,129-byte Annex B keyframe in 8.174 ms, packetized it into seven GLYT UDP datagrams (largest 1,253 bytes), decoded every datagram, and reconstructed an exact CRC32-matching access unit. This includes CPU BGRA-to-NV12 conversion and first-frame overhead but excludes live Desktop Duplication, network, Android decode, and render, so it remains a probe rather than an end-to-end benchmark distribution.

DXGI enumeration reported the two active displays at 2560x1440/165 Hz and 2560x1440/120 Hz. `DuplicateOutput` was denied to the current Codex automation desktop with `0x80070005`, so capture latency and full glass-to-glass latency still require an interactive-session run.
