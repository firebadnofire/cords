param(
    [Parameter(Mandatory = $true)][string]$SshHost,
    [ValidateRange(1024, 65535)][int]$ForwardPort = 58201,
    [string]$TestFilter = '',
    [string]$DesktopFixtureDirectory = '',
    [string]$TestExecutable = '',
    [switch]$SeedAudit,
    [switch]$DefaultsOnly,
    [switch]$ProfilePictures
)
# Disposable acceptance only. The caller must authorize Docker use on SshHost.
# Credentials exist only in memory/process environment and container metadata.
# No production container, named volume, or installation directory is modified.
$ErrorActionPreference = 'Stop'
if ($SeedAudit -and !$DesktopFixtureDirectory) { throw 'SeedAudit requires a disposable desktop fixture' }
if ($ProfilePictures -and !$DesktopFixtureDirectory) { throw 'ProfilePictures requires a disposable desktop fixture' }
if ($DefaultsOnly -and (!$DesktopFixtureDirectory -or $SeedAudit)) { throw 'DefaultsOnly requires a desktop fixture without SeedAudit' }
if ($TestExecutable) {
    if (!$DesktopFixtureDirectory -or ![IO.Path]::IsPathFullyQualified($TestExecutable) -or !(Test-Path -LiteralPath $TestExecutable -PathType Leaf)) {
        throw 'TestExecutable requires a desktop fixture and an existing absolute executable path'
    }
    Write-Output "Using prebuilt test executable (not a fresh source validation): $TestExecutable"
    Write-Output "SHA256: $((Get-FileHash -LiteralPath $TestExecutable -Algorithm SHA256).Hash)"
}
if ($SshHost -notmatch '^[a-zA-Z0-9_.-]+@[a-zA-Z0-9_.-]+$') {
    throw 'SshHost must be a user@host without shell syntax'
}
$fixtureName = 'cords-rbac-' + [guid]::NewGuid().ToString('N').Substring(0,12)
$fixturePassword = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
$fixtureTunnel = $null
$fixtureCreated = $false
$fixtureExit = 1
$previousUrl = $env:CORDS_TEST_DATABASE_URL
$previousDesktop = $env:CORDS_ROLE_DESKTOP_FIXTURE
$previousSeed = $env:CORDS_ROLE_DESKTOP_SEED_AUDIT
$previousDefaults = $env:CORDS_ROLE_DESKTOP_DEFAULTS_ONLY
$previousProfilePictures = $env:CORDS_ROLE_DESKTOP_PROFILE_PICTURES
try {
    $remoteScript = 'IFS= read -r fixture_password; printf "POSTGRES_PASSWORD=%s\n" "$fixture_password" | docker run -d --name ' + $fixtureName + ' --label cords.acceptance=role-policy --env-file /dev/stdin -e POSTGRES_USER=cords -e POSTGRES_DB=cords -p 127.0.0.1::5432 postgres@sha256:9b18b78397054fce88a9552e9d5a3ad5bb7fd258c5b3cc1c5028e46373d6ea8f'
    $null = $fixturePassword | ssh -o BatchMode=yes -o StrictHostKeyChecking=yes $SshHost $remoteScript
    if ($LASTEXITCODE -ne 0) { throw 'Disposable PostgreSQL creation failed' }
    $fixtureCreated = $true
    $portLine = ssh -o BatchMode=yes -o StrictHostKeyChecking=yes $SshHost "docker port $fixtureName 5432/tcp"
    if ($LASTEXITCODE -ne 0 -or $portLine -notmatch '^127\.0\.0\.1:(\d+)$') {
        throw 'Could not resolve loopback PostgreSQL port'
    }
    $remotePort = $Matches[1]
    $fixtureTunnel = Start-Process ssh -ArgumentList @('-N','-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ExitOnForwardFailure=yes','-L',"127.0.0.1:${ForwardPort}:127.0.0.1:$remotePort",$SshHost) -WindowStyle Hidden -PassThru
    $ready = $false
    for ($attempt=0; $attempt -lt 20; $attempt++) {
        # The image first runs a Unix-socket-only initialization server. Require
        # TCP readiness so a fast, already-built test binary cannot race startup.
        $null = ssh -o BatchMode=yes -o StrictHostKeyChecking=yes $SshHost "docker exec $fixtureName pg_isready -h 127.0.0.1 -U cords -d cords"
        if ($LASTEXITCODE -eq 0) { $ready = $true; break }
        Start-Sleep -Milliseconds 500
    }
    $fixtureTunnel.Refresh()
    if (!$ready -or $fixtureTunnel.HasExited) { throw 'PostgreSQL or verified SSH tunnel not ready' }
    Write-Output "Disposable fixture: $fixtureName; remote loopback port: $remotePort; local SSH tunnel: $ForwardPort"
    ssh -o BatchMode=yes -o StrictHostKeyChecking=yes $SshHost "docker exec $fixtureName postgres --version"
    if ($LASTEXITCODE -ne 0) { throw 'PostgreSQL version probe failed' }
    # PostgreSQL plaintext is confined to loopback at each endpoint of verified SSH.
    $env:CORDS_TEST_DATABASE_URL = 'postgres://cords:' + $fixturePassword + "@127.0.0.1:$ForwardPort/cords?sslmode=disable"
    Push-Location (Split-Path $PSScriptRoot -Parent)
    try {
        if ($DesktopFixtureDirectory) {
            $env:CORDS_ROLE_DESKTOP_SEED_AUDIT = if ($SeedAudit) { '1' } else { $null }
            $env:CORDS_ROLE_DESKTOP_DEFAULTS_ONLY = if ($DefaultsOnly) { '1' } else { $null }
            $env:CORDS_ROLE_DESKTOP_PROFILE_PICTURES = if ($ProfilePictures) { '1' } else { $null }
            $env:CORDS_ROLE_DESKTOP_FIXTURE = [IO.Path]::GetFullPath($DesktopFixtureDirectory)
            if ($TestExecutable) {
                & $TestExecutable desktop_role_acceptance_fixture --ignored --nocapture
            } else {
                cargo test --release -p cords-server-core --locked desktop_role_acceptance_fixture -- --ignored --nocapture
            }
        } elseif ($TestFilter) {
            cargo test --release -p cords-server-core --locked $TestFilter -- --include-ignored
        } else {
            cargo test --release -p cords-server-core --locked -- --include-ignored --skip desktop_role_acceptance_fixture
        }
        $fixtureExit = $LASTEXITCODE
    } finally { Pop-Location }
} finally {
    $env:CORDS_TEST_DATABASE_URL = $previousUrl
    $env:CORDS_ROLE_DESKTOP_FIXTURE = $previousDesktop
    $env:CORDS_ROLE_DESKTOP_SEED_AUDIT = $previousSeed
    $env:CORDS_ROLE_DESKTOP_DEFAULTS_ONLY = $previousDefaults
    $env:CORDS_ROLE_DESKTOP_PROFILE_PICTURES = $previousProfilePictures
    $fixturePassword = $null
    if ($null -ne $fixtureTunnel -and !$fixtureTunnel.HasExited) { Stop-Process -Id $fixtureTunnel.Id }
    if ($fixtureCreated) {
        ssh -o BatchMode=yes -o StrictHostKeyChecking=yes $SshHost "docker rm -fv $fixtureName"
        if ($LASTEXITCODE -ne 0) { throw "Fixture cleanup failed: $fixtureName" }
    }
}
exit $fixtureExit
