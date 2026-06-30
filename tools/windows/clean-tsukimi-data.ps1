<#
.SYNOPSIS
Removes data left in the Windows user profile by older, non-portable Tsukimi builds.

.DESCRIPTION
Only exact Tsukimi folder names under known per-user locations are considered.
The current portable application tree is always excluded. The script also
removes the legacy GSettings registry key used by Tsukimi before portable mode.

.PARAMETER Force
Skips the confirmation prompt. This does not broaden the set of paths removed.
#>

[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$folderNames = @(
    "tsukimi",
    "Tsukimi",
    "dev.tsukimi",
    "moe.tsukimi",
    "moe.tsuna.tsukimi",
    "io.github.tsukimi",
    "com.tsukimi"
)

# Older builds used GLib's Windows cache directory, while experimental builds
# may have used the standard LocalAppData, RoamingAppData, or Temp roots.
$roots = @(
    $env:LOCALAPPDATA,
    $env:APPDATA,
    $env:TEMP,
    $(if ($env:LOCALAPPDATA) {
        Join-Path $env:LOCALAPPDATA "Microsoft\Windows\INetCache"
    })
) | Where-Object { -not [string]::IsNullOrWhiteSpace($_) }

$portableRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$portableRoot = $portableRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
)

function Test-IsPortableTree {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    $candidate = [System.IO.Path]::GetFullPath($Path).TrimEnd(
        [System.IO.Path]::DirectorySeparatorChar,
        [System.IO.Path]::AltDirectorySeparatorChar
    )
    $separator = [System.IO.Path]::DirectorySeparatorChar

    return $candidate.Equals(
        $portableRoot,
        [System.StringComparison]::OrdinalIgnoreCase
    ) -or $portableRoot.StartsWith(
        "$candidate$separator",
        [System.StringComparison]::OrdinalIgnoreCase
    ) -or $candidate.StartsWith(
        "$portableRoot$separator",
        [System.StringComparison]::OrdinalIgnoreCase
    )
}

function Get-SafeFolderTarget {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return
    }

    if (Test-IsPortableTree -Path $Path) {
        Write-Warning "Skipping the current portable application tree: $Path"
        return
    }

    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        Write-Warning "Skipping reparse point: $Path"
        return
    }

    return $item.FullName
}

$folderTargets = @(
    foreach ($root in $roots) {
        foreach ($name in $folderNames) {
            $candidate = Join-Path $root $name
            $target = Get-SafeFolderTarget -Path $candidate
            if ($target) {
                $target
            }
        }

        # The installed schema uses /moe/tsukimi/ as its settings path.
        $schemaPathCandidate = Join-Path $root "moe\tsukimi"
        $schemaTarget = Get-SafeFolderTarget -Path $schemaPathCandidate
        if ($schemaTarget) {
            $schemaTarget
        }
    }
) | Sort-Object -Unique

# GLib's Windows registry backend stored this schema at
# HKEY_CURRENT_USER\Software\GSettings\moe\tsukimi.
$registryTargets = @(
    "Registry::HKEY_CURRENT_USER\Software\GSettings\moe\tsukimi",
    "Registry::HKEY_CURRENT_USER\Software\GSettings\moe.tsuna.tsukimi"
) | Where-Object { Test-Path -LiteralPath $_ }

$targets = @(
    $folderTargets | ForEach-Object {
        [PSCustomObject]@{ Type = "folder"; Path = $_ }
    }
    $registryTargets | ForEach-Object {
        [PSCustomObject]@{ Type = "registry key"; Path = $_ }
    }
)

if ($targets.Count -eq 0) {
    Write-Host "No old Tsukimi data was found."
    exit 0
}

Write-Host "The following old Tsukimi data will be removed:"
foreach ($target in $targets) {
    Write-Host ("  [{0}] {1}" -f $target.Type, $target.Path)
}

if (-not $Force) {
    $answer = Read-Host "Continue? Type Y to delete these paths"
    if ($answer -notmatch "^[Yy]$") {
        Write-Host "Cleanup cancelled. Nothing was deleted."
        exit 0
    }
}

$failed = $false
foreach ($target in $targets) {
    Write-Host ("Deleting [{0}]: {1}" -f $target.Type, $target.Path)
    try {
        Remove-Item -LiteralPath $target.Path -Recurse -Force
    } catch {
        $failed = $true
        Write-Warning ("Failed to delete {0}: {1}" -f $target.Path, $_.Exception.Message)
    }
}

if ($failed) {
    Write-Error "Cleanup finished with one or more errors."
    exit 1
}

Write-Host "Old Tsukimi data cleanup completed."
