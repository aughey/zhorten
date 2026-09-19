param(
    [string]$Username = 'admin',
    [Parameter(Mandatory = $true)]
    [string]$Password,
    [string]$Database = './data/zhorten.db',
    [string]$Address = '127.0.0.1:3000'
)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
& "$PSScriptRoot/build.ps1"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Push-Location $projectRoot
try {
    & "$projectRoot/target/debug/zhorten.exe" --username $Username --password $Password --database $Database --address $Address
}
finally {
    Pop-Location
}

