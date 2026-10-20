#Requires -Version 7.0
# Tests missing-dependency output without installing or compiling anything.
if ($env:OS -ne 'Windows_NT') { Write-Host 'Skipped: native Windows required'; return }
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$requirements=(Get-Content -Raw (Join-Path $root 'build-scripts/requirements/windows-client.ps1.txt')).Replace("`r`n","`n").Trim()
foreach($scenario in @('noninteractive','redirected','missing-tar')) {
    $start=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
    $start.UseShellExecute=$false
    $start.RedirectStandardOutput=$true
    $start.RedirectStandardError=$true
    $start.RedirectStandardInput=$true
    $start.Environment['PATH']=''
    $start.ArgumentList.Add('-NoProfile')
    $start.ArgumentList.Add('-Command')
    $scriptPath=(Join-Path $root 'build-scripts/windows-client.ps1').Replace("'","''")
    $command=if($scenario -eq 'missing-tar'){"function global:cargo {}; function global:rustc {}; & '$scriptPath' -NonInteractive"} elseif($scenario -eq 'redirected'){"& '$scriptPath'"} else {"& '$scriptPath' -NonInteractive"}
    $start.ArgumentList.Add($command)
    $process=[Diagnostics.Process]::Start($start)
    $process.StandardInput.Close()
    $outputTask=$process.StandardOutput.ReadToEndAsync()
    $errorTask=$process.StandardError.ReadToEndAsync()
    if(-not $process.WaitForExit(30000)){ $process.Kill(); throw "$scenario timed out" }
    $output=($outputTask.Result+ $errorTask.Result).Replace("`r`n","`n")

    if($process.ExitCode -eq 0){throw "$scenario unexpectedly succeeded"}
    if(-not $output.Contains('Requirements for windows-client.ps1:')){throw "$scenario missing requirements"}
    if(-not $output.Contains($requirements)){throw "$scenario missing complete requirements"}
    if($scenario -ne 'missing-tar' -and -not $output.Contains('Noninteractive run:')){throw "$scenario missing noninteractive safeguard"}
    if($scenario -eq 'missing-tar' -and -not $output.Contains('tar.exe is required')){throw 'Original tar failure lost'}
    Write-Host "PASS $scenario (exit $($process.ExitCode), full requirements, original error retained)"
}
