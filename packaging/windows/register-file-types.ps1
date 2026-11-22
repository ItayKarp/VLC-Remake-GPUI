<#
.SYNOPSIS
    Registers (or removes) VLC as a handler for media files and streaming URLs for the
    current user (S0.6.4), so it shows up in "Open with" and in Settings > Apps > Default apps.

.DESCRIPTION
    Writes per-user keys under HKCU\Software\Classes and HKCU\Software\RegisteredApplications:
    a ProgID that opens files with "<exe>" --started-from-file "%1", an OpenWithProgids entry per extension, and
    the Capabilities that "Default apps" lists. It doesn't take over any file type: Windows
    asks the user to pick the default. No administrator rights are needed.

    The installer (S9.4.1) should register the same keys. Keep $Extensions and $UrlSchemes in
    sync with src/app/file_types.rs; its tests check them.

.EXAMPLE
    .\register-file-types.ps1 -Exe C:\path\to\RedesignedVLC.exe
.EXAMPLE
    .\register-file-types.ps1 -Unregister
#>
param(
    [string]$Exe,
    [switch]$Unregister
)

$ErrorActionPreference = 'Stop'

$Extensions = @(
    '3gp', 'avi', 'flv', 'm2ts', 'm4v', 'mkv', 'mov', 'mp4', 'mpeg', 'mpg', 'mts', 'ogv', 'ts',
    'vob', 'webm', 'wmv',
    'aac', 'aiff', 'ape', 'flac', 'm4a', 'mka', 'mp3', 'oga', 'ogg', 'opus', 'wav', 'wma',
    'm3u', 'm3u8', 'pls', 'xspf'
)
$UrlSchemes = @('mms', 'rtmp', 'rtsp')

$ProgId = 'RedesignedVLC.Media'
$AppName = 'RedesignedVLC'
$Classes = 'HKCU:\Software\Classes'
$AppKey = "HKCU:\Software\$AppName"
$Capabilities = "$AppKey\Capabilities"
$Registered = 'HKCU:\Software\RegisteredApplications'

function Set-RegistryValue([string]$Path, [string]$Name, [string]$Value) {
    if (-not (Test-Path $Path)) { New-Item -Path $Path -Force | Out-Null }
    if ($Name -eq '') {
        Set-Item -Path $Path -Value $Value
    } else {
        New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType String -Force | Out-Null
    }
}

function Remove-RegistryKey([string]$Path) {
    if (Test-Path $Path) { Remove-Item -Path $Path -Recurse -Force }
}

function Unregister-FileTypes {
    Remove-RegistryKey "$Classes\$ProgId"
    foreach ($ext in $Extensions) {
        $openWith = "$Classes\.$ext\OpenWithProgids"
        if (Test-Path $openWith) {
            Remove-ItemProperty -Path $openWith -Name $ProgId -ErrorAction SilentlyContinue
        }
    }
    foreach ($scheme in $UrlSchemes) {
        # Remove a scheme only if this script claimed it.
        $command = Get-ItemProperty -Path "$Classes\$scheme\shell\open\command" -ErrorAction SilentlyContinue
        if ($command -and $command.'(default)' -like "*$AppName*") {
            Remove-RegistryKey "$Classes\$scheme"
        }
    }
    Remove-RegistryKey $AppKey
    Remove-ItemProperty -Path $Registered -Name $AppName -ErrorAction SilentlyContinue
    Write-Host 'Removed the file associations.'
}

function Register-FileTypes([string]$ExePath) {
    $open = "`"$ExePath`" --started-from-file `"%1`""

    Set-RegistryValue "$Classes\$ProgId" '' 'Media file'
    Set-RegistryValue "$Classes\$ProgId\DefaultIcon" '' "`"$ExePath`",0"
    Set-RegistryValue "$Classes\$ProgId\shell\open\command" '' $open

    Set-RegistryValue $Capabilities 'ApplicationName' 'VLC'
    Set-RegistryValue $Capabilities 'ApplicationDescription' 'VLC media player'
    foreach ($ext in $Extensions) {
        Set-RegistryValue "$Classes\.$ext\OpenWithProgids" $ProgId ''
        Set-RegistryValue "$Capabilities\FileAssociations" ".$ext" $ProgId
    }
    foreach ($scheme in $UrlSchemes) {
        # Claim a scheme only when nothing else handles it yet.
        if (-not (Test-Path "$Classes\$scheme")) {
            Set-RegistryValue "$Classes\$scheme" '' "URL:$scheme"
            Set-RegistryValue "$Classes\$scheme" 'URL Protocol' ''
            Set-RegistryValue "$Classes\$scheme\shell\open\command" '' $open
        }
        Set-RegistryValue "$Capabilities\URLAssociations" $scheme $ProgId
    }
    Set-RegistryValue $Registered $AppName "Software\$AppName\Capabilities"
    Write-Host "Registered $($Extensions.Count) file types. Pick VLC in Settings > Apps > Default apps."
}

if ($Unregister) {
    Unregister-FileTypes
} elseif ($Exe) {
    Register-FileTypes (Resolve-Path $Exe).Path
} else {
    throw 'Pass -Exe <path to RedesignedVLC.exe>, or -Unregister.'
}
