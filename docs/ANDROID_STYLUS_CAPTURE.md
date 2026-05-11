# Android Stylus Capture

The Android client captures raw stylus data from `MotionEvent` using Compose `pointerInteropFilter`.

Implementation entry points:

- `apps/android-client/src/main/java/com/glyphray/android/input/StylusModels.kt`
- `apps/android-client/src/main/java/com/glyphray/android/input/StylusDiagnosticsController.kt`
- `apps/android-client/src/main/java/com/glyphray/android/ui/screens/Screens.kt`

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

