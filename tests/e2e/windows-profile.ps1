param(
    [Parameter(Mandatory)][string]$Package,
    [Parameter(Mandatory)][string]$Sha256,
    [Parameter(Mandatory)][string]$Profile,
    [string]$CertificateAuthority,
    [switch]$ListAccounts
)
$ErrorActionPreference = 'Stop'
$archive = (Resolve-Path -LiteralPath $Package).Path
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $Sha256) {
    throw 'Complete Windows package checksum mismatch'
}
$profileRoot = [IO.Path]::GetFullPath($Profile)
if (-not ([IO.Path]::GetFileName($profileRoot)).StartsWith('profile-')) {
    throw 'Use a dedicated profile-* directory'
}
New-Item -ItemType Directory -Force -Path $profileRoot | Out-Null
$packageRoot = Join-Path (Split-Path $archive) ('package-' + $Sha256.Substring(0, 16))
if (-not (Test-Path -LiteralPath $packageRoot)) {
    Expand-Archive -LiteralPath $archive -DestinationPath $packageRoot
}
# Verify reused extracted packages as well as the incoming ZIP. A stale or
# modified DLL must not inherit trust merely from its extraction directory name.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$bundle = [IO.Compression.ZipFile]::OpenRead($archive)
try {
    foreach ($entry in $bundle.Entries) {
        if (-not $entry.Name) { continue }
        $destination = [IO.Path]::GetFullPath((Join-Path $packageRoot $entry.FullName))
        if (-not $destination.StartsWith($packageRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Package entry escapes extraction directory'
        }
        if (-not (Test-Path -LiteralPath $destination -PathType Leaf)) { throw "Missing package file: $($entry.FullName)" }
        $stream = $entry.Open()
        $algorithm = [Security.Cryptography.SHA256]::Create()
        try { $expected = [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-', '') }
        finally { $stream.Dispose(); $algorithm.Dispose() }
        if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ine $expected) {
            throw "Extracted package checksum mismatch: $($entry.FullName)"
        }
    }
} finally { $bundle.Dispose() }
$executables = @(Get-ChildItem -LiteralPath $packageRoot -Recurse -Filter Cords.exe)
if ($executables.Count -ne 1) { throw 'Expected exactly one Cords executable' }
$start = [Diagnostics.ProcessStartInfo]::new($executables[0].FullName)
$start.UseShellExecute = $false
$start.WorkingDirectory = $executables[0].DirectoryName
$start.Environment['CORDS_CLIENT_STATE'] = $profileRoot
$start.Environment['QT_ACCESSIBILITY'] = '1'
foreach ($entry in @{APPDATA='app-data'; LOCALAPPDATA='local-data'; TEMP='tmp'; TMP='tmp'; QML_DISK_CACHE_PATH='cache/qml'}.GetEnumerator()) {
    $location = Join-Path $profileRoot $entry.Value
    New-Item -ItemType Directory -Force -Path $location | Out-Null
    $start.Environment[$entry.Key] = $location
}
if ($CertificateAuthority) {
    $start.Environment['CORDS_CLIENT_CA'] = (Resolve-Path -LiteralPath $CertificateAuthority).Path
} else {
    $start.Environment.Remove('CORDS_CLIENT_CA') | Out-Null
}
if ($ListAccounts) {
    $start.Arguments = '--list-accounts'
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit(30000)) { $process.Kill(); throw 'Account listing timed out' }
    if ($process.ExitCode -ne 0) { throw $stderr.Result }
    $stdout.Result
} else {
    $process = [Diagnostics.Process]::Start($start)
    @{pid=$process.Id; executable=$start.FileName; profile=$profileRoot; package_sha256=$Sha256} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $profileRoot 'test-process.json')
    Write-Output "Started test-owned Cords PID $($process.Id); profile $profileRoot"
}
