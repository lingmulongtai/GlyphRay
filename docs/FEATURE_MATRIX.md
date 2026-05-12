# Feature Matrix

This document tracks product controls that must exist on both the Android client and desktop host before beta.

## Video And Session Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Resolution | Client can select 1080p, 1440p, or host native and send `EncoderConfig`. Host stores requested config. | Wire stored config into the live capture/encode loop. Add host UI override. |
| Refresh rate | Client can request 60/90/120 fps in `EncoderConfig`. | Clamp against host display and encoder capability. |
| Bitrate | Client can request bitrate presets in `EncoderConfig`. Transport has adaptive bitrate foundation. | Combine manual bitrate with adaptive bitrate controller. |
| Color space | Protocol now carries `ColorSpace` and Android can request sRGB, Display P3, Rec.709, or Rec.2020 PQ. | Wire through encoder, decoder, and render surface metadata. |
| Codec | Protocol and Android settings support H.264, H.265, and AV1 preferences. H.264 remains the first real implementation target. | Add host encoder capability negotiation and client decoder fallback. |
| Host-side controls | Host currently logs and stores client encoder config. | Add native host UI and console/CLI override for encoder config. |

## Input Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Pen input | Android captures stylus/finger/mouse `MotionEvent` samples and streams compact batches. Windows host can decode and optionally inject native pen input. | Validate pressure, tilt, hover, barrel button, and eraser in creative apps. |
| Touch input | Finger samples are captured; Android now exposes touch mode choices: direct touch, trackpad, gesture assist. | Implement real touch gesture translation for tap, drag, right click, scroll, and zoom. |
| Bluetooth keyboard | Android remote surface can receive `KeyEvent`, map common keys to Windows virtual keys, and send `KeyboardInput`. Windows host can decode and opt-in inject with `SendInput` for approved clients. | Add layout-aware text input, IME handling, host UI permission prompts, and broader key coverage. |
| Windows key / PrintScreen | Android session overlay sends Win and PrintScreen key packets. Host can inject them when `GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1` is set. | Add visible host/client safety indicators and per-device permission controls for privileged keys. |
| Fullscreen client mode | Android session can hide the bottom navigation for a fullscreen/focus mode. | Hide Android system bars and add gesture escape affordance. |

## Host Startup And Login

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Start at boot/login | Not implemented yet. | Add Windows installer option for user-logon startup and a background service supervisor. |
| Connect before Windows login | Not implemented. Windows capture/input APIs are constrained by interactive desktop, secure desktop, and user session boundaries. | Split into service broker plus per-user capture agent, document what is impossible on the lock screen, and require explicit security review. |
| Run without active desktop session | Not implemented. | Investigate service + agent model, but do not promise creative-app control before user login until native API validation is complete. |
