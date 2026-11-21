# Captures evidence after an interactive session, for the report back.
# Read-only: it reads the profile the app used and prints an inventory.
#
#   pwsh -File scripts/after-run.ps1
#   pwsh -File scripts/after-run.ps1 -OutFile session-evidence.json

param([string]$OutFile)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $root 'target\debug\pfl-cli.exe'
$data = Join-Path $env:LOCALAPPDATA 'patent-forge-lite'
$db = Join-Path $data 'projects.sqlite3'

if (-not (Test-Path $db)) { throw "no profile at $db — did the app open a different profile?" }

$json = & $cli inspect --database $db
if ($LASTEXITCODE -ne 0) { throw "could not read the profile" }
Write-Host $json

$info = $json | ConvertFrom-Json
$runs = @($info.projects[0].runs).Count
Write-Host "`nSummary" -ForegroundColor White
Write-Host ("  projects     : {0}" -f $info.project_count)
Write-Host ("  runs         : {0}" -f $runs)
Write-Host ("  lock present : {0}" -f (Test-Path (Join-Path $data 'projects.lock')))
Write-Host ("  profile size : {0:N1} MiB" -f ((Get-Item $db).Length / 1MB))
Get-ChildItem $data | Select-Object Name, Length, LastWriteTime | Format-Table -AutoSize | Out-String | Write-Host

if ($OutFile) {
    $json | Set-Content -Path $OutFile -Encoding UTF8
    Write-Host "Evidence written to $OutFile" -ForegroundColor Green
}
