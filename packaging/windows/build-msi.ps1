<#
.SYNOPSIS
    Builds the Windows installer (S9.4.1): dist\RedesignedVLC-<version>-windows-x64.msi, holding
    the app, libVLC and its plugins, signed with Authenticode when a certificate is given.

.DESCRIPTION
    Builds the release binary, stages it with libVLC from an unpacked VLC 3 64-bit ZIP from
    videolan.org (libvlc.dll, libvlccore.dll, vlc-cache-gen.exe and plugins\, leaving out
    packaging\libvlc\excluded-plugins.txt, S9.4.4), signs the executable, builds RedesignedVLC.wxs
    with WiX 5 and signs the MSI.

    Needs the Rust toolchain with Visual Studio's C++ tools, WiX 5 (`dotnet tool install -g wix`,
    then `wix extension add -g WixToolset.Util.wixext`) and, to sign, signtool from the Windows
    SDK with the certificate in the certificate store.

    A new certificate has no SmartScreen reputation: Windows warns about the installer until
    enough people have run it, or from the start with an EV certificate.

.EXAMPLE
    .\packaging\windows\build-msi.ps1 -Vlc C:\src\vlc-3.0.23
.EXAMPLE
    .\packaging\windows\build-msi.ps1 -Vlc C:\src\vlc-3.0.23 -CertificateThumbprint 0123ABCD...
#>
param(
    # The unpacked VLC ZIP: the folder holding libvlc.dll and plugins\.
    [Parameter(Mandatory)] [string]$Vlc,
    # The SHA-1 thumbprint of the code-signing certificate. Unsigned without it.
    [string]$CertificateThumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    # Cargo features to add, such as 'perf'.
    [string]$Features
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path "$PSScriptRoot\..\..").Path
$Target = 'x86_64-pc-windows-msvc'
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "$Root\target" }
$Stage = "$TargetDir\msi-stage"
$Out = "$Root\dist"

function Get-Version {
    $line = Get-Content "$Root\Cargo.toml" | Where-Object { $_ -match '^version = "(.*)"' } | Select-Object -First 1
    [void]($line -match '^version = "(.*)"')
    $Matches[1]
}

function Invoke-Checked([string]$Program, [string[]]$Arguments) {
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}

function Build-Binary {
    $arguments = @('build', '--manifest-path', "$Root\Cargo.toml", '--release', '--target', $Target)
    if ($Features) { $arguments += @('--features', $Features) }
    Invoke-Checked cargo $arguments
    "$TargetDir\$Target\release\RedesignedVLC.exe"
}

function Get-ExcludedPlugins {
    Get-Content "$Root\packaging\libvlc\excluded-plugins.txt" |
        ForEach-Object { $_.Trim() } |
        Where-Object { $_ -and -not $_.StartsWith('#') }
}

# The installer writes plugins.dat itself (vlc-cache-gen, in RedesignedVLC.wxs), since Windows
# Installer doesn't keep the file times the ZIP's cache was made for.
function Copy-Runtime {
    foreach ($file in 'libvlc.dll', 'libvlccore.dll', 'vlc-cache-gen.exe') {
        if (-not (Test-Path "$Vlc\$file")) { throw "No $file in $Vlc" }
        Copy-Item "$Vlc\$file" $Stage
    }
    Copy-Item "$Vlc\plugins" "$Stage\plugins" -Recurse
    Remove-Item "$Stage\plugins\plugins.dat" -ErrorAction SilentlyContinue
    $removed = 0
    foreach ($name in Get-ExcludedPlugins) {
        Get-ChildItem "$Stage\plugins" -Recurse -File -Filter "lib${name}_plugin.dll" |
            ForEach-Object { Remove-Item $_.FullName; $removed++ }
    }
    Write-Host "Left out $removed plugins"
}

function Invoke-Sign([string]$Path) {
    if (-not $CertificateThumbprint) { return }
    Invoke-Checked signtool @('sign', '/sha1', $CertificateThumbprint, '/fd', 'SHA256',
        '/tr', $TimestampUrl, '/td', 'SHA256', '/d', 'VLC', $Path)
}

function Build-Msi([string]$Version) {
    # MSI versions are numbers only: 0.2.0-pr12.3 installs as 0.2.0.
    $numeric = ($Version -split '-')[0]
    $msi = "$Out\RedesignedVLC-$Version-windows-x64.msi"
    Invoke-Checked wix @('build', "$Root\packaging\windows\RedesignedVLC.wxs", '-arch', 'x64',
        '-ext', 'WixToolset.Util.wixext', '-d', "Version=$numeric", '-d', "Stage=$Stage",
        '-d', "Icon=$Root\assets\app-icon\icon.ico", '-o', $msi)
    Invoke-Sign $msi
    $msi
}

$version = Get-Version
$exe = Build-Binary
Remove-Item $Stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item $Stage, $Out -ItemType Directory -Force | Out-Null
Copy-Item $exe $Stage
Invoke-Sign "$Stage\RedesignedVLC.exe"
Copy-Runtime
$msi = Build-Msi $version
Write-Host "Built $msi"
