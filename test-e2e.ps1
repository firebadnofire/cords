param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
$ErrorActionPreference = 'Stop'
$python = if ($env:CORDS_PYTHON) { $env:CORDS_PYTHON } else { 'python' }
& $python (Join-Path $PSScriptRoot 'tests/e2e/run.py') @Arguments
exit $LASTEXITCODE
