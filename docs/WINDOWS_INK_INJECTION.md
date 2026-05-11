# Windows Ink Injection

Native Windows pen injection is the key GlyphRay feature.

## API Path

The Windows host uses the modern synthetic pointer path:

- `CreateSyntheticPointerDevice`
- `InjectSyntheticPointerInput`
- `PT_PEN`
- `POINTER_TYPE_INFO`
- `POINTER_PEN_INFO`

Implementation entry point:

- `hosts/windows-host/src/input/mod.rs`
- `hosts/windows-host/src/input/win32_pen.rs`

## Preserved Fields

From Android stylus samples, the injector maps:

- x/y location through `glyphray-core` coordinate mapping
- pressure through `PressureMapper::to_windows_pressure`
- tilt X/Y
- rotation/orientation
- hover vs contact state
- down/move/up/cancel
- pointer id

Barrel button and eraser flags are represented in the protocol and need final mapping validation against Windows creative apps.

## Diagnostic Tool

Run on Windows:

```powershell
cargo run -p glyphray-pen-diagnostics
```

The tool injects a short synthetic pressure stroke. Validate it in apps that expose pressure and tilt, such as Krita, OneNote, Clip Studio Paint, or Photoshop.

## Limitations To Validate

- Exact `POINTER_PEN_INFO` button flag mapping.
- App-specific pressure range behavior.
- Hover behavior across Win32, UWP, and Wintab-aware apps.
- Multi-monitor high-DPI coordinate correctness.
- Whether elevated target apps require host privilege changes.

