param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root "target\\release\\ara-cli.exe"
$debug = Join-Path $root "target\\debug\\ara-cli.exe"
$cargoExe = Join-Path $env:USERPROFILE ".cargo\\bin\\cargo.exe"

if (Test-Path $release) {
    & $release @Args
    exit $LASTEXITCODE
}

if (Test-Path $debug) {
    & $debug @Args
    exit $LASTEXITCODE
}

Push-Location $root
try {
    & $cargoExe run -p ara-cli -- @Args
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
