param(
    [string]$Configuration = "release",
    [string]$OutputDir = "dist\windows",
    [string]$Version = "",
    [string]$SigningCertificatePath = $env:WINDOWS_SIGNING_CERTIFICATE,
    [string]$SigningCertificatePassword = $env:WINDOWS_SIGNING_CERTIFICATE_PASSWORD,
    [string]$TimestampUrl = "http://timestamp.digicert.com",
    [switch]$PayloadOnly
)

$ErrorActionPreference = "Stop"

$root = Resolve-Path "$PSScriptRoot\..\..\.."
$versionFile = Join-Path $root "VERSION"
if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = (Get-Content -LiteralPath $versionFile -Raw).Trim()
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version must use numeric major.minor.patch form for MSI packaging: $Version"
}
$out = Join-Path $root $OutputDir
$payload = Join-Path $out "payload"
New-Item -ItemType Directory -Force -Path $payload | Out-Null

function Find-SignTool {
    $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $kitsRoot = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    $candidate = Get-ChildItem -Path "$kitsRoot\*\x64\signtool.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending |
        Select-Object -First 1
    if (-not $candidate) {
        throw "signtool.exe was not found in PATH or the Windows SDK."
    }
    return $candidate.FullName
}

function Sign-Artifact([string]$Path) {
    if ([string]::IsNullOrWhiteSpace($SigningCertificatePath)) {
        return
    }
    if (-not (Test-Path -LiteralPath $SigningCertificatePath)) {
        throw "Signing certificate was not found: $SigningCertificatePath"
    }

    $signTool = Find-SignTool
    & $signTool sign /fd SHA256 /td SHA256 /tr $TimestampUrl /f $SigningCertificatePath /p $SigningCertificatePassword $Path
    if ($LASTEXITCODE -ne 0) {
        throw "Signing failed for $Path"
    }
}

Push-Location $root
try {
    cargo build -p glyphray-windows-host --bin glyphray-windows-host --profile $Configuration
    Copy-Item "target\$Configuration\glyphray-windows-host.exe" (Join-Path $payload "glyphray-windows-host.exe") -Force
    Sign-Artifact (Join-Path $payload "glyphray-windows-host.exe")

    if ($PayloadOnly) {
        Write-Host "Payload staged at $payload"
        return
    }

    if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
        throw "WiX CLI was not found. Install WiX Toolset v4 with: dotnet tool install --global wix"
    }

    $msiPath = Join-Path $out "GlyphRayHost-$Version.msi"
    wix build `
        "tools\packaging\windows\GlyphRayHost.wxs" `
        "tools\packaging\windows\GlyphRayHostComponents.wxs" `
        -d PayloadDir="$payload" `
        -d ProductVersion="$Version" `
        -out $msiPath
    Sign-Artifact $msiPath
}
finally {
    Pop-Location
}
