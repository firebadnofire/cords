param(
    [string]$OutputRoot = $env:CORDS_QT_OUTPUT_ROOT
)
# Dot-source before native builds/tests. Only this PowerShell process is changed.
# Existing caches, credentials, personal vaults and installed tools are untouched.
$ErrorActionPreference = 'Stop'
if (!$OutputRoot) {
    if (Test-Path -LiteralPath 'E:\') { $OutputRoot = 'E:\CordsBuild' }
    else { throw 'Set CORDS_QT_OUTPUT_ROOT or pass -OutputRoot on a drive with sufficient free space.' }
}
if (![IO.Path]::IsPathFullyQualified($OutputRoot)) { throw 'OutputRoot must be an absolute dedicated output directory.' }
$qtOutput = [IO.Path]::GetFullPath($OutputRoot)
$qtRepo = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
if ($qtOutput.TrimEnd('\','/') -eq [IO.Path]::GetPathRoot($qtOutput).TrimEnd('\','/') -or
    $qtOutput -eq $qtRepo -or $qtRepo.StartsWith($qtOutput.TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'OutputRoot must not be a drive root, repository root or its ancestor.'
}
foreach ($qtFolder in @('', 'tmp', 'cargo', 'cache', 'cache\cargo-home', 'cache\pip', 'cache\qml', 'build', 'packages', 'fixtures', 'evidence')) {
    $null = New-Item -ItemType Directory -Force -Path (Join-Path $qtOutput $qtFolder)
}
$env:CORDS_QT_OUTPUT_ROOT = $qtOutput
$env:CARGO_TARGET_DIR = Join-Path $qtOutput 'cargo'
$env:CARGO_HOME = Join-Path $qtOutput 'cache\cargo-home'
$env:TEMP = Join-Path $qtOutput 'tmp'
$env:TMP = $env:TEMP
$env:TMPDIR = $env:TEMP
$env:XDG_CACHE_HOME = Join-Path $qtOutput 'cache'
$env:PIP_CACHE_DIR = Join-Path $qtOutput 'cache\pip'
$env:QML_DISK_CACHE_PATH = Join-Path $qtOutput 'cache\qml'
$env:PYTHONDONTWRITEBYTECODE = '1'
Write-Output "Qt outputs: $qtOutput (build, cargo, cache, tmp, packages, fixtures, evidence). Pass build/package paths explicitly to build-client-qt.py."
