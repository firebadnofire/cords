[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Write-Step {
    param([Parameter(Mandatory)][string]$Message)
    Write-Host "==> $Message"
}

function Require-Command {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)][string]$Guidance
    )
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' is unavailable; $Guidance"
    }
}

function Invoke-Native {
    param(
        [Parameter(Mandatory)][string]$Command,
        [Parameter(ValueFromRemainingArguments)][string[]]$Arguments
    )
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE`: $Command $($Arguments -join ' ')"
    }
}

if ($env:OS -ne 'Windows_NT') {
    throw 'windows-client.ps1 requires a Windows host'
}

$scriptDirectory = Split-Path -Parent $PSCommandPath
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $scriptDirectory '..'))
$cargoToml = Join-Path $repositoryRoot 'Cargo.toml'
$cargoText = Get-Content -Raw -LiteralPath $cargoToml
$versionMatch = [regex]::Match(
    $cargoText,
    '(?ms)^\[workspace\.package\]\s*.*?^version\s*=\s*"([^"]+)"'
)
if (-not $versionMatch.Success -or $versionMatch.Groups[1].Value -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$') {
    throw 'Could not read a safe workspace version from Cargo.toml'
}
$version = $versionMatch.Groups[1].Value

$uiDirectory = Join-Path $repositoryRoot 'bins\cords-client\ui'
# npm ci replaces node_modules, including executables that Windows locks while
# the development server is running. Fail before partially removing dependencies.
$esbuildDirectory = Join-Path $uiDirectory 'node_modules\@esbuild'
$runningEsbuild = @(Get-CimInstance Win32_Process -Filter "Name = 'esbuild.exe'" | Where-Object {
    $_.ExecutablePath -and $_.ExecutablePath.StartsWith(
        "$esbuildDirectory\", [StringComparison]::OrdinalIgnoreCase
    )
})
if ($runningEsbuild.Count -gt 0) {
    $processIds = ($runningEsbuild | ForEach-Object { $_.ProcessId }) -join ', '
    throw "Cords frontend dependencies are in use by esbuild (PID: $processIds). Stop this checkout's Vite/Tauri development server with Ctrl+C, then rerun this script. npm ci cannot replace a running executable on Windows."
}

$targetDirectory = if ($env:CARGO_TARGET_DIR) {
    [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
} else {
    Join-Path $repositoryRoot 'target'
}

$nativeArchitecture = if (-not [string]::IsNullOrWhiteSpace($env:PROCESSOR_ARCHITEW6432)) {
    $env:PROCESSOR_ARCHITEW6432
} else {
    $env:PROCESSOR_ARCHITECTURE
}
if ([string]::IsNullOrWhiteSpace($nativeArchitecture)) {
    throw 'Windows did not expose PROCESSOR_ARCHITECTURE'
}
$architecture = switch ($nativeArchitecture.ToUpperInvariant()) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { throw "Unsupported Windows architecture: $nativeArchitecture" }
}
$rustTarget = switch ($architecture) {
    'x86_64' { 'x86_64-pc-windows-msvc' }
    'aarch64' { 'aarch64-pc-windows-msvc' }
}

Require-Command cargo 'install the Rust toolchain declared by rust-toolchain.toml'
Require-Command rustc 'install the Rust toolchain declared by rust-toolchain.toml'
Require-Command 'npm.cmd' 'install Node.js 24 and npm'
Require-Command tar.exe 'enable the Windows archive utility'

$rustVersion = (& rustc -vV) -join "`n"
if ($LASTEXITCODE -ne 0 -or $rustVersion -notmatch "(?m)^host: $([regex]::Escape($rustTarget))$") {
    throw "The active Rust host must be $rustTarget"
}

$distDirectory = if ($env:CORDS_DIST_DIR) {
    [IO.Path]::GetFullPath($env:CORDS_DIST_DIR)
} else {
    Join-Path $repositoryRoot 'dist'
}
[IO.Directory]::CreateDirectory($distDirectory) | Out-Null

$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ("cords-windows-client-" + [guid]::NewGuid().ToString('N'))
$packageName = "cords-client-windows-$architecture"
$packageRoot = Join-Path $temporaryRoot $packageName
[IO.Directory]::CreateDirectory($packageRoot) | Out-Null

try {
    Write-Step "Building Cords Windows client $version for $architecture"
    Push-Location $uiDirectory
    try {
        Invoke-Native -Command npm.cmd -Arguments @('ci')
        Invoke-Native -Command npm.cmd -Arguments @('run', 'build')
    } finally {
        Pop-Location
    }

    Invoke-Native -Command cargo -Arguments @(
        'build', '--locked', '--release', '--manifest-path', $cargoToml,
        '--target', $rustTarget, '--target-dir', $targetDirectory,
        '-p', 'cords-client', '--features', 'custom-protocol'
    )
    $client = Join-Path $targetDirectory "$rustTarget\release\cords-client.exe"
    if (-not (Test-Path -LiteralPath $client -PathType Leaf) -or (Get-Item -LiteralPath $client).Length -eq 0) {
        throw "Windows client was not produced: $client"
    }

    Copy-Item -LiteralPath $client -Destination (Join-Path $packageRoot 'Cords.exe')
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'README.md') -Destination $packageRoot
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'LICENSE') -Destination $packageRoot
    $codecNotices = Join-Path $packageRoot 'licenses\avatar-codec'
    [IO.Directory]::CreateDirectory((Split-Path $codecNotices -Parent)) | Out-Null
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'assets\avatar-codec\licenses') -Destination $codecNotices -Recurse
    # A portable Cargo build does not run Tauri's resource bundler. Preserve the
    # same resource layout used by release-mode app.path().resource_dir().
    $migrationSource = Join-Path $repositoryRoot 'migrations\sqlite'
    $migrationDestination = Join-Path $packageRoot 'migrations\sqlite'
    [IO.Directory]::CreateDirectory($migrationDestination) | Out-Null
    $migrationFiles = @(Get-ChildItem -LiteralPath $migrationSource -Filter '*.sql' -File)
    if ($migrationFiles.Count -eq 0) { throw 'SQLite migrations are missing from the source tree' }
    foreach ($migration in $migrationFiles) {
        if ($migration.Length -eq 0) { throw "Empty SQLite migration: $($migration.Name)" }
        Copy-Item -LiteralPath $migration.FullName -Destination $migrationDestination
    }

    $artifact = Join-Path $distDirectory "cords-client-windows-$architecture-$version.zip"
    $checksum = "$artifact.sha256"
    $legacyArtifact = Join-Path $distDirectory "cords-client-windows-$architecture-$version.tar.gz"
    Remove-Item -LiteralPath $artifact, $checksum -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $legacyArtifact, "$legacyArtifact.sha256" -Force -ErrorAction SilentlyContinue
    Write-Step "Packaging portable Windows client ZIP"
    Invoke-Native -Command tar.exe -Arguments @('-a', '-C', $temporaryRoot, '-cf', $artifact, $packageName)
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf) -or (Get-Item -LiteralPath $artifact).Length -eq 0) {
        throw "Windows client archive was not produced: $artifact"
    }

    $hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText($checksum, "$hash  $([IO.Path]::GetFileName($artifact))`n", [Text.UTF8Encoding]::new($false))
    if (-not (Test-Path -LiteralPath $checksum -PathType Leaf)) {
        throw "Checksum was not produced: $checksum"
    }
    Write-Step "Created $artifact"
} finally {
    if (Test-Path -LiteralPath $temporaryRoot) {
        $resolvedTemporaryRoot = [IO.Path]::GetFullPath($temporaryRoot)
        $expectedTemporaryParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolvedTemporaryRoot.StartsWith($expectedTemporaryParent, [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolvedTemporaryRoot) -notmatch '^cords-windows-client-[0-9a-f]{32}$') {
            throw "Refusing to remove an unexpected staging directory: $resolvedTemporaryRoot"
        }
        # Windows scanners may briefly retain a directory entry after its last
        # file is removed. Retry only this validated, run-specific staging path.
        for ($attempt = 1; $attempt -le 3; $attempt++) {
            if (-not (Test-Path -LiteralPath $resolvedTemporaryRoot)) { break }
            try {
                Remove-Item -LiteralPath $resolvedTemporaryRoot -Recurse -Force -ErrorAction Stop
                break
            } catch {
                if ($attempt -eq 3) {
                    throw "Failed to clean build staging directory '$resolvedTemporaryRoot' after 3 attempts: $($_.Exception.Message)"
                }
                Write-Warning "Staging cleanup failed; retrying ($attempt/3): $($_.Exception.Message)"
                Start-Sleep -Milliseconds 250
            }
        }
    }
}

