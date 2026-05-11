# GlyphRay Product Spec

## Product Goal

GlyphRay is a low-latency remote creative desktop application for artists, designers, illustrators, and tablet-first creative workflows.

The primary scenario is using an Android tablet, especially a Samsung Galaxy Tab S11 Ultra with S Pen, as a high-quality remote pen display for a Windows 10/11 or macOS 13+ host.

GlyphRay must not copy Parsec source code, UI assets, brand, icons, protocol, wording, or visual design. Parsec is only a benchmark for perceived latency, setup simplicity, and reliability.

## Differentiator

Android stylus input is transmitted as high-frequency stylus samples and injected on Windows as native pen input using Windows pointer injection APIs. The product must preserve pressure, hover, tilt, orientation, button state, eraser state, timestamps, and batched historical samples when the platform exposes them.

## Target Users

- Digital artists using Clip Studio Paint, Krita, Photoshop, OneNote, Blender Grease Pencil, and similar tools.
- Designers who need responsive remote access from a tablet.
- Users who want a simple LAN-first remote display without a required cloud account.

## MVP Scope

- Android client with host list, pairing, connection, remote session, pen settings, video settings, security, and diagnostics screens.
- Windows host with pairing, connection permission, display selection, encoder settings, security, diagnostics, screen capture, and native pen injection.
- macOS host structure prepared for Phase 2.
- Local network pairing and encrypted transport.
- Binary protocol for session control, video, and high-priority input.

## Non-Goals For MVP

- Cloud account system.
- Relay server.
- Commercial installer polish.
- Full macOS pen parity with Windows Ink.
- Clipboard by default.

