# Roadmap

Current overall progress estimate: 31%.

## Milestone 1

- [x] Create monorepo structure.
- [x] Add Rust workspace crates.
- [x] Add binary protocol frame and stylus schema.
- [x] Add coordinate mapping tests.
- [x] Add pressure curve tests.
- [x] Add security/session helper tests.
- [x] Add transport simulation tests.
- [x] Add Android Compose skeleton.
- [x] Add Android stylus diagnostics screen.
- [x] Add Windows host skeleton.
- [x] Add Windows pen injection wrapper.
- [x] Add Windows pen diagnostic CLI.
- [x] Add macOS host shell.
- [x] Add initial docs and CI.

## Milestone 2

- [x] Implement Windows monitor enumeration.
- [ ] Implement Windows Graphics Capture or Desktop Duplication capture path.
- [x] Add encoder trait with H.264 low-latency configuration.
- [ ] Add at least one hardware encoder path or a clean software fallback.
- [x] Add Android MediaCodec H.264 decode pipeline.
- [x] Add UDP datagram packet format for transport packets.
- [x] Add video frame chunking and reassembly utilities.
- [ ] Stream video over LAN using the transport abstraction.
- [x] Add latency overlay prepared for `glyphray-telemetry`.

## Milestone 3

- [ ] Connect Android stylus packets to Windows native pen injection.
- [ ] Validate pressure, tilt, hover, barrel button, and eraser in creative apps.
- [ ] Add drawing area calibration UI.
- [ ] Harden multi-monitor and high-DPI mapping.

## Milestone 4

- [ ] Add encrypted production transport.
- [ ] Add platform secret stores.
- [ ] Add reconnect and adaptive bitrate.
- [ ] Add host diagnostics UI.
- [ ] Add installer packaging.

## Milestone 5

- [ ] Complete macOS host capture and encode path.
- [ ] Add audio.
- [ ] Add optional relay architecture.
- [ ] Prepare beta release.
