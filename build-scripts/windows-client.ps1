[CmdletBinding()]
param(
    [string]$QtPrefix = $env:CORDS_QT_PREFIX,
    [string]$BuildDirectory = $env:CORDS_QT_BUILD_DIR,
    [string]$CMake = $env:CORDS_CMAKE,
    [ValidateSet('release', 'dev')][string]$Profile = 'release',
    [switch]$NonInteractive
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script:RequirementsPrinted = $false
function Show-BuildRequirements {
    if ($script:RequirementsPrinted) { return }
    $script:RequirementsPrinted = $true
    Write-Host "`nRequirements for windows-client.ps1:"
    Get-Content -LiteralPath (Join-Path $PSScriptRoot 'requirements\windows-client.ps1.txt') | ForEach-Object { Write-Host $_ }
    Write-Host "Full setup and controls: $(Join-Path $PSScriptRoot 'README.md')"
}
trap { Show-BuildRequirements; throw $_ }
function Invoke-Native {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Command failed ($LASTEXITCODE): $Command $($Arguments -join ' ')" }
}
function Update-BuildPath {
    $paths = [Collections.Generic.List[string]]::new()
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($value in @([Environment]::GetEnvironmentVariable('PATH', 'Machine'), [Environment]::GetEnvironmentVariable('PATH', 'User'), $env:PATH)) {
        foreach ($entry in ($value -split ';')) {
            $entry = [Environment]::ExpandEnvironmentVariables($entry.Trim())
            if ($entry -and $seen.Add($entry)) { $paths.Add($entry) }
        }
    }
    $env:PATH = $paths -join ';'
}
function Confirm-PrerequisiteInstallation {
    while ($true) {
        $answer = (Read-Host 'Install the missing prerequisite? [Y/n]').Trim()
        if (-not $answer -or $answer -match '^(y|yes)$') { return $true }
        if ($answer -match '^(n|no)$') { return $false }
        Write-Host 'Enter Y or N, or press Enter for Yes.'
    }
}
function Install-MissingPrerequisite {
    param([string]$Message, [string]$PackageId, [string[]]$Commands,
        [string[]]$InstallArguments = @(), [string]$ExistingCommand = '')
    Write-Host "`n$Message"
    Show-BuildRequirements
    Write-Host 'Installation commands:'
    foreach ($command in $Commands) { Write-Host "  $command" }
    $interactive = [Environment]::UserInteractive
    try { $interactive = $interactive -and -not [Console]::IsInputRedirected } catch { $interactive = $false }
    $ciRun = $env:CI -and $env:CI -notmatch '^(0|false)$'
    if ($NonInteractive -or $ciRun -or -not $interactive) {
        throw "$Message Noninteractive run: install the prerequisites using the commands above, then retry."
    }
    if (-not (Confirm-PrerequisiteInstallation)) { throw "$Message Installation declined; no packages were installed." }
    if ($ExistingCommand -and (Get-Command $ExistingCommand -ErrorAction SilentlyContinue)) {
        Write-Host "Using existing $ExistingCommand from PATH."
        return
    }
    $winget = Get-Command winget.exe -ErrorAction SilentlyContinue
    if (-not $winget) { throw 'WinGet is unavailable. Install Microsoft App Installer from https://apps.microsoft.com/detail/9NBLGGH4NNS1, then retry.' }
    Invoke-Native $winget.Source (@('install', '--exact', '--id', $PackageId, '--source', 'winget') + $InstallArguments)
    Update-BuildPath
}
function Require-BuildCommand {
    param([string]$Name, [string]$PackageId)
    if (Get-Command $Name -ErrorAction SilentlyContinue) { return }
    Install-MissingPrerequisite "Required command is missing: $Name" $PackageId @(
        "winget install --exact --id $PackageId --source winget"
    )
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Installation completed but $Name is still unavailable. Check PATH or reopen PowerShell and retry."
    }
}
if ($env:OS -ne 'Windows_NT') { throw 'windows-client.ps1 requires native Windows/MSVC' }
$repositoryRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$cargoText = Get-Content -Raw -LiteralPath (Join-Path $repositoryRoot 'Cargo.toml')
$versionMatch = [regex]::Match($cargoText, '(?ms)^\[workspace\.package\]\s*.*?^version\s*=\s*"([^"]+)"')
if (-not $versionMatch.Success -or $versionMatch.Groups[1].Value -notmatch '^[0-9]+\.[0-9]+\.[0-9]+([-.+][0-9A-Za-z.-]+)?$') {
    throw 'Cannot read a safe workspace version'
}
$version = $versionMatch.Groups[1].Value
$nativeArchitecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$architecture = switch ($nativeArchitecture.ToUpperInvariant()) {
    'AMD64' { 'x86_64' }
    'ARM64' { 'aarch64' }
    default { throw "Unsupported Windows architecture: $nativeArchitecture" }
}
foreach ($command in @('cargo', 'rustc')) { Require-BuildCommand $command 'Rustlang.Rustup' }
if (-not (Get-Command tar.exe -ErrorAction SilentlyContinue)) { throw 'Windows tar.exe is required; restore the Windows System32 directory to PATH or repair the Windows installation.' }
$rustHost = (& rustc -vV) -join "`n"
if ($LASTEXITCODE -ne 0 -or $rustHost -notmatch "(?m)^host: $architecture-pc-windows-msvc$") {
    throw "Use the native $architecture MSVC Rust toolchain"
}
$python = if ($env:CORDS_PYTHON) { $env:CORDS_PYTHON } else { 'python' }
if (-not (Get-Command $python -ErrorAction SilentlyContinue)) {
    Write-Host "Selected Python '$python' is unavailable; checking python on PATH."
    Require-BuildCommand 'python' 'Python.Python.3.13'
    $python = 'python'
}
if (-not $CMake) {
    $found = Get-Command cmake -ErrorAction SilentlyContinue
    if ($found) { $CMake = $found.Source }
}
for ($attempt = 0; $attempt -lt 2; $attempt++) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $vcComponent = if ($architecture -eq 'aarch64') { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }
        $installation = & $vswhere -latest -products '*' -requires $vcComponent -property installationPath
        if ($LASTEXITCODE -ne 0) { throw 'Visual Studio discovery failed' }
        if ($installation) {
            if (-not $CMake) { $CMake = Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe' }
            if (-not $env:VCINSTALLDIR) { $env:VCINSTALLDIR = (Join-Path $installation 'VC') + '\' }
        }
    }
    if ($env:VCINSTALLDIR -and (Test-Path -LiteralPath $env:VCINSTALLDIR -PathType Container)) { break }
    if ($attempt -eq 1) { throw 'Visual Studio installation completed but C++ tools are unavailable. Check VCINSTALLDIR or modify Visual Studio to add the C++ workload, then retry.' }
    $armComponent = if ($architecture -eq 'aarch64') { ' --add Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { '' }
    $override = '--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' + $armComponent
    Install-MissingPrerequisite 'Visual Studio 2022 C++ tools and redistributables are required.' 'Microsoft.VisualStudio.2022.BuildTools' @(
        ('winget install --exact --id Microsoft.VisualStudio.2022.BuildTools --source winget --override "' + $override + '"')
    ) @('--override', $override)
    $env:VCINSTALLDIR = $null
}
if (-not $CMake -or -not (Test-Path -LiteralPath $CMake -PathType Leaf)) {
    Install-MissingPrerequisite 'CMake >=3.24 is required.' 'Kitware.CMake' @(
        'winget install --exact --id Kitware.CMake --source winget'
    )
    $found = Get-Command cmake.exe -ErrorAction SilentlyContinue
    if (-not $found) { throw 'CMake installation completed but cmake.exe is unavailable. Reopen PowerShell or pass -CMake with its installed path.' }
    $CMake = $found.Source
}
$qtArchitecture = if ($architecture -eq 'aarch64') { 'win64_msvc2022_arm64_cross_compiled' } else { 'win64_msvc2022_64' }
$qtSdkFolder = if ($architecture -eq 'aarch64') { 'msvc2022_arm64' } else { 'msvc2022_64' }
$qtInstallRoot = Join-Path $env:LOCALAPPDATA 'Cords\Qt'
$defaultQtPrefix = Join-Path $qtInstallRoot "6.10.3\$qtSdkFolder"
if (-not $QtPrefix) { $QtPrefix = $env:QT_ROOT_DIR }
if (-not $QtPrefix) {
    $qmake = Get-Command qmake.exe -ErrorAction SilentlyContinue
    if ($qmake) {
        $QtPrefix = & $qmake.Source -query QT_INSTALL_PREFIX
        if ($LASTEXITCODE -ne 0) { throw 'Qt SDK discovery failed' }
    }
}
if (-not $QtPrefix -and (Test-Path -LiteralPath (Join-Path $defaultQtPrefix 'bin\qmake.exe') -PathType Leaf)) { $QtPrefix = $defaultQtPrefix }
if (-not $QtPrefix -or -not (Test-Path -LiteralPath (Join-Path $QtPrefix 'bin\qmake.exe') -PathType Leaf)) {
    Install-MissingPrerequisite 'Qt 6.10.3 MSVC SDK is required. WinGet supplies aqt; aqt then downloads the pinned Qt SDK.' 'miurahr.aqtinstall' @(
        'winget install --exact --id miurahr.aqtinstall --source winget --version 3.3.0'
        ('aqt install-qt windows desktop 6.10.3 ' + $qtArchitecture + ' --outputdir "$env:LOCALAPPDATA\Cords\Qt" --archives qtbase qtdeclarative qtsvg qttools qttranslations')
    ) @('--version', '3.3.0') 'aqt'
    $aqt = Get-Command aqt -ErrorAction SilentlyContinue
    if (-not $aqt) { throw 'aqt installation completed but aqt is unavailable. Reopen PowerShell and retry.' }
    Write-Host "==> Installing Qt 6.10.3 ($qtArchitecture) into $qtInstallRoot"
    Invoke-Native $aqt.Source @('install-qt', 'windows', 'desktop', '6.10.3', $qtArchitecture,
        '--outputdir', $qtInstallRoot, '--archives', 'qtbase', 'qtdeclarative', 'qtsvg', 'qttools', 'qttranslations')
    $QtPrefix = $defaultQtPrefix
    if (-not (Test-Path -LiteralPath (Join-Path $QtPrefix 'bin\qmake.exe') -PathType Leaf)) { throw "Qt installation did not produce $QtPrefix\bin\qmake.exe; inspect the installer output." }
    $env:CORDS_QT_PREFIX = $QtPrefix
    Write-Host "==> Using Qt SDK: $QtPrefix (automatically detected on subsequent runs)"
}
if (-not $PSBoundParameters.ContainsKey('Profile') -and $env:CORDS_QT_PROFILE) { $Profile = $env:CORDS_QT_PROFILE }
if ($Profile -notin @('release', 'dev')) { throw 'CORDS_QT_PROFILE must be release or dev' }
if (-not $BuildDirectory) { $BuildDirectory = Join-Path $repositoryRoot "target\qt\windows-$architecture" }
$BuildDirectory = [IO.Path]::GetFullPath($BuildDirectory)
$resolvedRepository = $repositoryRoot.TrimEnd('\') + '\'
if ($repositoryRoot.StartsWith($BuildDirectory.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or $BuildDirectory -eq $repositoryRoot) {
    throw 'BuildDirectory must be a dedicated output directory'
}
$cargoTarget = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $BuildDirectory 'cargo' }
$distDirectory = if ($env:CORDS_DIST_DIR) { [IO.Path]::GetFullPath($env:CORDS_DIST_DIR) } else { Join-Path $repositoryRoot 'dist' }
[IO.Directory]::CreateDirectory($distDirectory) | Out-Null
$stagingParent = [IO.Path]::GetFullPath((Join-Path $BuildDirectory 'packaging-runs'))
$stagingRoot = Join-Path $stagingParent ([guid]::NewGuid().ToString('N'))
$packageName = "cords-client-windows-$architecture"
$packageRoot = Join-Path $stagingRoot $packageName
$artifact = Join-Path $distDirectory "$packageName-$version.zip"
$temporaryArchive = Join-Path $distDirectory ('.cords-qt-' + [guid]::NewGuid().ToString('N') + '.zip')
try {
    Write-Host "==> Building native Qt Cords $version ($architecture, $Profile)"
    Invoke-Native $python @((Join-Path $repositoryRoot 'scripts\build-client-qt.py'),
        '--qt-prefix', $QtPrefix, '--build-dir', $BuildDirectory, '--cargo-target', $cargoTarget,
        '--package-dir', $packageRoot, '--profile', $Profile, '--cmake', $CMake, '--tests')
    foreach ($required in @('Cords.exe', 'Qt6Core.dll', 'share\cords\migrations\sqlite')) {
        if (-not (Test-Path -LiteralPath (Join-Path $packageRoot $required))) { throw "Qt package is missing $required" }
    }
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'README.md') -Destination $packageRoot
    Copy-Item -LiteralPath (Join-Path $repositoryRoot 'LICENSE') -Destination $packageRoot
    Write-Host '==> Packaging the native executable, Qt runtime, plugins and migrations'
    Invoke-Native tar.exe @('-a', '-C', $stagingRoot, '-cf', $temporaryArchive, $packageName)
    if (-not (Test-Path -LiteralPath $temporaryArchive) -or (Get-Item -LiteralPath $temporaryArchive).Length -eq 0) { throw 'Native ZIP was not produced' }
    Move-Item -LiteralPath $temporaryArchive -Destination $artifact -Force
    $hash = (Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText("$artifact.sha256", "$hash  $([IO.Path]::GetFileName($artifact))`n", [Text.UTF8Encoding]::new($false))
    Write-Host "==> Created $artifact; extract the complete folder and run Cords.exe"
} finally {
    if (Test-Path -LiteralPath $temporaryArchive) { Remove-Item -LiteralPath $temporaryArchive -Force }
    if (Test-Path -LiteralPath $stagingRoot) {
        $resolvedStaging = [IO.Path]::GetFullPath($stagingRoot)
        if (-not $resolvedStaging.StartsWith($stagingParent.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
            [IO.Path]::GetFileName($resolvedStaging) -notmatch '^[0-9a-f]{32}$') { throw "Unsafe staging cleanup target: $resolvedStaging" }
        Remove-Item -LiteralPath $resolvedStaging -Recurse -Force -ErrorAction Stop
    }
}
