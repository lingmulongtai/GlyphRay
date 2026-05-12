# Android Stylus Capture

The Android client captures raw stylus data from `MotionEvent` using Compose `pointerInteropFilter`.

Implementation entry points:

- `apps/android-client/src/main/java/com/glyphray/android/input/StylusModels.kt`
- `apps/android-client/src/main/java/com/glyphray/android/input/StylusDiagnosticsController.kt`
- `apps/android-client/src/main/java/com/glyphray/android/ui/screens/Screens.kt`
- `apps/android-client/src/main/java/com/glyphray/android/input/StylusPacketEncoder.kt`
- `apps/android-client/src/main/java/com/glyphray/android/input/StylusStreamController.kt`
- `apps/android-client/src/main/java/com/glyphray/android/network/TransportPacketCodec.kt`
- `apps/android-client/src/main/java/com/glyphray/android/network/StylusLanBridgeController.kt`
- `apps/android-client/src/main/java/com/glyphray/android/ui/video/RemoteDisplayView.kt`

## Captured Fields

- x/y coordinates
- pressure
- tilt (`AXIS_TILT`)
- orientation
- hover enter/move/exit
- button state
- eraser tool type
- tool type
- historical batched points
- event timestamps in nanoseconds
- distance when available

## Samsung S Pen Notes

Samsung devices can provide high-frequency historical samples. GlyphRay keeps those samples instead of collapsing them into a single point so Windows injection can preserve stroke fidelity.

Palm rejection is currently represented as a settings surface. Milestone 3 should add policy around finger rejection while the stylus is hovering or in contact.

`StylusPacketEncoder` writes the compact `GLYS` packet format documented in `docs/PROTOCOL.md`.

`TransportPacketCodec` wraps those `GLYS` payloads in the UDP `GLYT` datagram shape used by the Rust host backend. It also understands Video-channel `VideoFrame` datagrams and exposes a small QoS send queue that mirrors the host's input/control-heavy schedule.

`StylusLanBridgeController` connects a discovered host to the remote-session drawing surface. It converts `MotionEvent` data into compact stylus packets immediately, then sends UDP datagrams through the QoS-aware background worker so Android never performs network I/O on the UI thread.

The session receiver can route incoming Video-channel fragments into `RemoteVideoStreamController`, which reassembles H.264 access units and feeds MediaCodec when a decoder surface is attached.
