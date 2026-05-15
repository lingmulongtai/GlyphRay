# Windows Startup And Service Model

GlyphRay now supports a user-logon startup path for development and early beta builds.

## Implemented User-Logon Startup

The Windows host exposes:

```powershell
cargo run -p glyphray-windows-host -- startup status
cargo run -p glyphray-windows-host -- startup enable
cargo run -p glyphray-windows-host -- startup disable
```

The implementation writes the current host executable command to:

```text
HKCU\Software\Microsoft\Windows\CurrentVersion\Run
```

The registered command is:

```text
"<path-to-glyphray-windows-host.exe>" serve
```

This is intentionally per-user. It does not require administrator rights and does not run before the user logs in.

The live host console also accepts:

```text
startup status
startup enable
startup disable
```

## Why Pre-Login Is Not Claimed Yet

Remote creative-desktop control before Windows login is constrained by Windows session isolation, the secure desktop, input-injection boundaries, GPU capture access, and user consent. GlyphRay should not promise pre-login drawing-app control until a service/agent architecture has been validated against these platform rules.

## Production Direction

The production model should split the host into:

- A small Windows service broker for discovery, trusted-device state, update checks, and launching the per-user agent.
- A per-user interactive agent for screen capture, encoder access, tray UI, permission prompts, and input injection.
- Installer-managed startup options, firewall rules, uninstall cleanup, and signed binaries.

The service must not inject arbitrary input into a locked or secure desktop. Any future lock-screen behavior needs a separate security review and clear product copy.
