# Feature Matrix

This document tracks product controls that must exist on both the Android client and desktop host before beta.

## Video And Session Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Resolution | Client can select 1080p, 1440p, or host native and send `EncoderConfig`. Host now restarts the video pump from approved client config, but capture remains display-native until scaler support lands. | Add scaling/cropping stage before encode and clamp against encoder capability. |
| Refresh rate | Client can request 60/90/120 fps in `EncoderConfig`; host pump uses the requested FPS as its frame interval after clamping to 30-120. | Clamp against actual monitor refresh and encoder capability. |
| Bitrate | Client can request bitrate presets in `EncoderConfig`; host pump applies the requested bitrate to effective encoder settings. Transport has adaptive bitrate foundation. | Combine manual bitrate with adaptive bitrate controller. |
| Color space | Protocol now carries `ColorSpace` and Android can request sRGB, Display P3, Rec.709, or Rec.2020 PQ. | Wire through encoder, decoder, and render surface metadata. |
| Codec | Protocol and Android settings support H.264, H.265, and AV1 preferences. H.264 remains the first real implementation target. | Add host encoder capability negotiation and client decoder fallback. |
| Display selection | Android receives host `DisplayInfo`, lets the user select a host display in video settings, and sends that display id with stylus, touch, and mouse input packets. Windows runtime maps input against the selected/default display geometry when available. | Add calibration profile persistence per host display and validate high-DPI/rotation behavior on hardware. |
| Host-side controls | Host console supports `encoder status`, `encoder override <width>x<height> <fps> <kbps>`, and `encoder clear`; live pump restarts from host override or approved client config. | Add native host UI and persistent presets. |

## Input Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Pen input | Android captures stylus `MotionEvent` samples and streams compact batches. Windows host can decode and optionally inject native pen input. | Validate pressure, tilt, hover, barrel button, and eraser in creative apps. |
| Touch input | Android finger input now uses a distinct `TouchInputBatch` protocol path. Touch mode can preserve direct native touch, translate one-finger movement into trackpad-style mouse motion with tap-to-click, or convert two-finger gestures into wheel deltas. Windows host can opt-in inject native `PT_TOUCH` events with `GLYPHRAY_ENABLE_TOUCH_INJECTION=1`. | Validate multi-touch gestures in Windows apps and add client-visible touch calibration presets. |
| Bluetooth keyboard | Android remote surface can receive `KeyEvent`, map common keys to Windows virtual keys, and send `KeyboardInput`. Windows host can decode and opt-in inject with `SendInput` for approved clients. | Add layout-aware text input, IME handling, host UI permission prompts, and broader key coverage. |
| Bluetooth mouse | Android mouse `MotionEvent` now sends `MouseInput`. Windows host can opt-in inject cursor/buttons/wheel with `GLYPHRAY_ENABLE_MOUSE_INJECTION=1`. | Add relative pointer-lock mode, high-resolution wheel handling, and host permission UI. |
| Game controller | Android gamepad buttons/axes now send `GamepadInput`. Windows host decodes controller reports. | Add virtual gamepad backend such as ViGEm/virtual HID and per-device controller mapping UI. |
| Windows key / PrintScreen | Android session overlay sends Win and PrintScreen key packets. Host can inject them when `GLYPHRAY_ENABLE_KEYBOARD_INJECTION=1` is set. | Add visible host/client safety indicators and per-device permission controls for privileged keys. |
| Fullscreen client mode | Android session hides the bottom navigation and enters immersive system-bar hiding mode. Active sessions keep the screen awake. | Add a visible gesture escape affordance and per-device fullscreen preference. |

## Network Compatibility

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| LAN broadcast discovery | Implemented with `GLYD` UDP advertisements. | Keep as default local mode. |
| Tailscale / WireGuard-style overlay networks | Android can manually add and save a host by Tailscale IP or MagicDNS name because broadcast discovery usually does not cross overlays. Saved endpoints are restored into the host list on app start. UDP control/input traffic uses the same ports once the endpoint is known. | Add QR pairing that includes overlay IP and NAT/MTU diagnostics. |

## Distribution

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Windows installer | WiX v4 MSI packaging files and `tools/packaging/windows/build-msi.ps1` are present. | Add code signing, service/agent installation, startup option, and uninstall cleanup. |
| macOS installer | `pkgbuild` packaging script exists at `tools/packaging/macos/build-pkg.sh`. | Convert the SwiftPM executable into a signed/notarized app bundle with permission onboarding. |
| Android Play Store | Debug APK builds today; Play Store internal testing is the target release path. | Add release signing config outside repo, privacy policy, data safety form, app icon assets, and closed-testing checklist. |

## Host Startup And Login

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Start at boot/login | Not implemented yet. | Add Windows installer option for user-logon startup and a background service supervisor. |
| Connect before Windows login | Not implemented. Windows capture/input APIs are constrained by interactive desktop, secure desktop, and user session boundaries. | Split into service broker plus per-user capture agent, document what is impossible on the lock screen, and require explicit security review. |
| Run without active desktop session | Not implemented. | Investigate service + agent model, but do not promise creative-app control before user login until native API validation is complete. |
