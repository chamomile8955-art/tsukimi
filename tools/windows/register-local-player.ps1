# Registers a per-user Open With choice. Never changes UserChoice or other players.
[CmdletBinding()]
param([switch]$Unregister)

$ErrorActionPreference = "Stop"
$executable = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\tsukimi.exe"))
$extensionFile = Join-Path $PSScriptRoot "video-extensions.json"
if (-not (Test-Path -LiteralPath $extensionFile)) {
    $extensionFile = Join-Path $PSScriptRoot "..\..\resources\video-extensions.json"
}
$extensions = @(Get-Content -LiteralPath $extensionFile -Raw | ConvertFrom-Json)
foreach ($extension in $extensions) {
    if ($extension -notmatch '^[a-z0-9]+$') { throw "Invalid video extension" }
}
$progId = "Tsukimi.LocalVideo"
$classes = "Software\Classes"
$capabilities = "Software\TsukimiLocalPlayer\Capabilities"
$command = '"{0}" --local-player -- "%1"' -f $executable
$registry = [Microsoft.Win32.Registry]::CurrentUser

function Set-Value([string]$Path, [string]$Name, [string]$Value) {
    $key = $registry.CreateSubKey($Path)
    try { $key.SetValue($Name, $Value, [Microsoft.Win32.RegistryValueKind]::String) }
    finally { $key.Dispose() }
}

function Get-Command([string]$Path) {
    $key = $registry.OpenSubKey("$Path\shell\open\command")
    if ($null -eq $key) { return $null }
    try { return $key.GetValue("") } finally { $key.Dispose() }
}

# Do not remove registrations belonging to another portable copy.
$owner = Get-Command "$classes\$progId"
if ($Unregister) {
    if ($owner -ne $command) { throw "This portable copy does not own the current registration." }
    foreach ($extension in $extensions) {
        $key = $registry.OpenSubKey("$classes\.$extension\OpenWithProgids", $true)
        if ($null -ne $key) {
            try { $key.DeleteValue($progId, $false) } finally { $key.Dispose() }
        }
    }
    if ((Get-Command "$classes\Applications\tsukimi.exe") -eq $command) {
        $registry.DeleteSubKeyTree("$classes\Applications\tsukimi.exe", $false)
    }
    $registry.DeleteSubKeyTree("$classes\$progId", $false)
    $registry.DeleteSubKeyTree("Software\TsukimiLocalPlayer", $false)
    $key = $registry.OpenSubKey("Software\RegisteredApplications", $true)
    if ($null -ne $key) {
        try { $key.DeleteValue("Tsukimi Local Player", $false) } finally { $key.Dispose() }
    }
    Write-Host "Removed this copy's local player registration. Other players and UserChoice were left unchanged."
} else {
    if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw "tsukimi.exe not found: $executable" }
    $existingApp = Get-Command "$classes\Applications\tsukimi.exe"
    if ($null -ne $existingApp -and $existingApp -ne $command) {
        throw "Another Tsukimi copy is registered. Unregister that copy first."
    }
    if ($null -ne $owner -and $owner -ne $command) {
        throw "Another Tsukimi copy owns the video registration. Unregister it first."
    }
    Set-Value "$classes\$progId" "" "Tsukimi Video"
    Set-Value "$classes\$progId" "NoRecentDocs" ""
    Set-Value "$classes\$progId\DefaultIcon" "" ('"{0}",0' -f $executable)
    Set-Value "$classes\$progId\shell\open\command" "" $command
    Set-Value "$classes\Applications\tsukimi.exe" "FriendlyAppName" "Tsukimi Local Player"
    Set-Value "$classes\Applications\tsukimi.exe" "NoRecentDocs" ""
    Set-Value "$classes\Applications\tsukimi.exe\shell\open\command" "" $command
    Set-Value $capabilities "ApplicationName" "Tsukimi Local Player"
    Set-Value $capabilities "ApplicationDescription" "Local video playback without application history"
    foreach ($extension in $extensions) {
        Set-Value "$classes\.$extension\OpenWithProgids" $progId ""
        Set-Value "$classes\Applications\tsukimi.exe\SupportedTypes" ".$extension" ""
        Set-Value "$capabilities\FileAssociations" ".$extension" $progId
    }
    Set-Value "Software\RegisteredApplications" "Tsukimi Local Player" $capabilities
    Write-Host "Registered Tsukimi Local Player. Choose it in Open With > Choose another app > Always."
}

if (-not ("TsukimiAssociations" -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class TsukimiAssociations {
    [DllImport("shell32.dll")]
    public static extern void SHChangeNotify(uint eventId, uint flags, IntPtr item1, IntPtr item2);
}
'@
}
[TsukimiAssociations]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
