param(
    [string]$Configuration = "release",
    [string]$OutputDir = "dist\windows"
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path "$PSScriptRoot\..\..\.."
$out = Join-Path $root $OutputDir
$payload = Join-Path $out "payload"
New-Item -ItemType Directory -Force -Path $payload | Out-Null

Push-Location $root
try {
    cargo build -p glyphray-windows-host --bin glyphray-windows-host --profile $Configuration
    Copy-Item "target\$Configuration\glyphray-windows-host.exe" (Join-Path $payload "glyphray-windows-host.exe") -Force

    if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
        Write-Host "WiX CLI was not found. Install WiX Toolset v4 and run: dotnet tool install --global wix"
        Write-Host "Payload staged at $payload"
        exit 0
    }

    wix build `
        "tools\packaging\windows\GlyphRayHost.wxs" `
        "tools\packaging\windows\GlyphRayHostComponents.wxs" `
        -d PayloadDir="$payload" `
        -out (Join-Path $out "GlyphRayHost-0.1.0.msi")
}
finally {
    Pop-Location
}
