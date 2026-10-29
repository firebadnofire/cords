<#
.SYNOPSIS
Removes generated Cords build and package output, like make clean.
.DESCRIPTION
Cleans target/ and dist/ plus configured Windows builder output directories.
Use -WhatIf to preview. Stop running builds and clients before cleaning.
Source files, installed SDKs and application data are preserved.
#>
[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'Medium')]
param(
    [string]$BuildDirectory = $env:CORDS_QT_BUILD_DIR,
    [string]$CargoTargetDirectory = $env:CARGO_TARGET_DIR,
    [string]$DistDirectory = $env:CORDS_DIST_DIR
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($env:OS -ne 'Windows_NT') { throw 'clean-windows.ps1 requires Windows' }
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..')).TrimEnd('\')
$defaultTargets = @((Join-Path $repositoryRoot 'target'), (Join-Path $repositoryRoot 'dist'))
$targets = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)

# Validate every target before deleting anything. Custom output directories must
# be outside the source tree or inside the two standard generated directories.
foreach ($candidate in @($defaultTargets) + @($BuildDirectory, $CargoTargetDirectory, $DistDirectory)) {
    if ([string]::IsNullOrWhiteSpace($candidate)) { continue }
    $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($candidate)
    $resolved = [IO.Path]::GetFullPath($resolved).TrimEnd('\')
    $volumeRoot = [IO.Path]::GetPathRoot($resolved).TrimEnd('\')
    if ($resolved -eq $volumeRoot -or $resolved -eq $repositoryRoot -or
        $repositoryRoot.StartsWith($resolved + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Unsafe cleanup target (root or repository ancestor): $resolved"
    }
    if ($resolved.StartsWith($repositoryRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
        $generated = $false
        foreach ($allowed in $defaultTargets) {
            if ($resolved -eq $allowed -or $resolved.StartsWith($allowed + '\', [StringComparison]::OrdinalIgnoreCase)) {
                $generated = $true
            }
        }
        if (-not $generated) { throw "Cleanup target is inside the source tree: $resolved" }
    }
    # Reject junctions/symlinks in the path and output tree so deletion cannot
    # traverse into a directory other than the validated output directory.
    $ancestor = $resolved
    while ($ancestor) {
        if (Test-Path -LiteralPath $ancestor) {
            $item = Get-Item -LiteralPath $ancestor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Cleanup target uses a junction or symbolic link: $ancestor"
            }
        }
        $ancestor = [IO.Path]::GetDirectoryName($ancestor)
    }
    if (Test-Path -LiteralPath $resolved) {
        if (-not (Test-Path -LiteralPath $resolved -PathType Container)) {
            throw "Cleanup target is not a directory: $resolved"
        }
        foreach ($item in Get-ChildItem -LiteralPath $resolved -Force -Recurse) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Cleanup target contains a junction or symbolic link: $($item.FullName)"
            }
        }
    }
    $null = $targets.Add($resolved)
}

foreach ($target in $targets) {
    if (-not (Test-Path -LiteralPath $target)) {
        Write-Host "Already clean: $target"
        continue
    }
    if ($PSCmdlet.ShouldProcess($target, 'Remove generated build/package output recursively')) {
        Write-Host "Cleaning: $target"
        Remove-Item -LiteralPath $target -Recurse -Force
    }
}
