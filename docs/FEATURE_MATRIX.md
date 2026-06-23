# Feature Matrix

This document tracks product controls that must exist on both the Android client and desktop host before beta.

## Video And Session Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Resolution | Client sends `EncoderConfig`. Windows restarts its pump from approved config; macOS applies even-sized requested output dimensions through ScreenCaptureKit and VideoToolbox, clamped to the selected display. | Add Windows scaling/cropping and validate macOS quality/performance on hardware. |
| Refresh rate | Client requests 60/90/120 fps. Windows uses the requested interval; macOS applies a 1-120 fps clamped ScreenCaptureKit/VideoToolbox interval. | Clamp against measured monitor and encoder capabilities. |
| Bitrate | Windows and macOS apply approved client bitrate requests; macOS clamps them to 1-100 Mbps. Transport has adaptive bitrate foundations. | Feed live bandwidth estimates into both encoders. |
| Color space | Protocol now carries `ColorSpace` and Android can request sRGB, Display P3, Rec.709, or Rec.2020 PQ. | Wire through encoder, decoder, and render surface metadata. |
| Codec | Protocol and Android settings support H.264, H.265, and AV1 preferences. H.264 remains the first real implementation target. | Add host encoder capability negotiation and client decoder fallback. |
| Display selection | Windows and macOS send `DisplayInfo`; Android selects a host display and returns that id in video/input packets. macOS applies the display id to ScreenCaptureKit. | Persist calibration per display and validate high-DPI/rotation behavior on hardware. |
| Host-side controls | Host console supports `encoder status`, `encoder override <width>x<height> <fps> <kbps>`, `encoder save`, `encoder clear`, and named preset commands: `encoder preset list`, `encoder preset save <name>`, `encoder preset apply <name>`, and `encoder preset delete <name>`. Live pump restarts from host override, named preset, or approved client config. Saved host encoder overrides reload on backend startup. Android persists video/input preferences locally. | Add native host UI/tray controls and expose presets in a visual settings panel. |

## Pairing And Permission

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Host approval | New devices must prove the salted six-digit one-time code before console/native approval is enabled. Windows can show an opt-in Win32 permission dialog with `GLYPHRAY_ENABLE_PERMISSION_DIALOG=1`; macOS displays the pending code in SwiftUI. Approved devices are recorded in host settings. Returning devices skip the code but must match the saved Android public-key fingerprint and complete signed `AuthChallenge` / `AuthResponse` proof. | Promote Windows approval and trusted-device controls into a tray/settings UI and add optional QR presentation. |
| Trusted devices | Host settings persist trusted-device records with id, label, last peer, Android public-key SHA-256 fingerprint, DER public key when available, approval time, and input permission flags. Host console supports `trust list`, `trust forget <id>`, and `trust clear`. | Add per-device permission editing and expose the list in the host UI. |

## Input Controls

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Pen input | Android captures stylus `MotionEvent` samples and streams compact batches. Windows host can decode and optionally inject native pen input. | Validate pressure, tilt, hover, barrel button, and eraser in creative apps. |
| Touch input | Android finger input uses a distinct `TouchInputBatch` path. Touch mode supports native touch, trackpad-style mouse movement with tap-to-click, and two-finger wheel gestures. Windows injects `PT_TOUCH` only for encrypted peers with touch permission. | Validate multi-touch gestures in Windows apps and add client-visible touch calibration presets. |
| Bluetooth keyboard | Android maps common keys to Windows virtual keys. Windows injects with `SendInput`; macOS maps the shared virtual keys/modifiers to CGEvents after encryption. | Add layout-aware text input, IME handling, and broader key coverage. |
| Bluetooth mouse | Android sends `MouseInput`; Windows and macOS inject cursor, primary/secondary/middle buttons, and wheel only after encrypted-session establishment. | Add relative pointer-lock mode and high-resolution wheel validation. |
| Game controller | Android gamepad buttons/axes now send `GamepadInput`. Windows host decodes controller reports, enforces per-device permission, routes them through a virtual gamepad bridge, and normalizes to an XInput-style report boundary. | Link a production ViGEm/virtual HID native backend, validate a signed driver path, and add per-device controller mapping UI. |
| Windows key / PrintScreen | Android session overlay sends Win and PrintScreen key packets. Host injects them only for encrypted peers with keyboard permission. | Add visible host/client safety indicators for privileged keys. |
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
| macOS installer | Packaging creates a `.app`, `.pkg`, and zip; release CI supports Developer ID signing and notarization secrets and blocks unsigned tagged releases. | Validate with production certificates and complete permission onboarding UX. |
| Android Play Store | CI builds release APK/AAB and supports secret-backed signing; internal testing is the target release path. | Upload a signed AAB, finish privacy/data-safety forms, and complete closed testing. |

## Host Startup And Login

| Feature | Current status | Next implementation step |
| --- | --- | --- |
| Start at boot/login | Windows host supports per-user startup-at-login through `startup status`, `startup enable`, and `startup disable`, backed by the HKCU Run key. | Add installer UI, tray UI control, and a background service supervisor. |
| Connect before Windows login | Not implemented. Windows capture/input APIs are constrained by interactive desktop, secure desktop, and user session boundaries. | Split into service broker plus per-user capture agent, document what is impossible on the lock screen, and require explicit security review. |
| Run without active desktop session | Not implemented. | Investigate service + agent model, but do not promise creative-app control before user login until native API validation is complete. |
