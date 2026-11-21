# Seeds the Patent Forge Lite desktop profile with a completed four-workflow
# project, so the first interactive session has real history to open, select,
# re-export and resume instead of an empty list.
#
#   pwsh -File scripts/seed-profile.ps1            # deterministic recorded replay
#   pwsh -File scripts/seed-profile.ps1 -Live      # real local model (slow, ~12 min)
#   pwsh -File scripts/seed-profile.ps1 -Force     # replace an existing profile
#
# It refuses to overwrite an existing profile unless -Force is given, and it
# verifies the seeded database with `pfl-cli inspect` before reporting success.

param(
    [switch]$Live,
    [switch]$Force,
    [string]$Source,
    [string]$Endpoint = 'http://127.0.0.1:11434',
    [string]$Model = 'phi4-mini:latest'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $root 'target\debug\pfl-cli.exe'
$data = Join-Path $env:LOCALAPPDATA 'patent-forge-lite'
$db = Join-Path $data 'projects.sqlite3'

if (-not (Test-Path $cli)) {
    throw "missing $cli — run: cargo build -p pfl-cli"
}
if ((Test-Path $db) -and -not $Force) {
    Write-Host "A profile already exists at $db" -ForegroundColor Yellow
    Write-Host "Re-run with -Force to replace it, or delete that directory first."
    exit 1
}

$staging = Join-Path ([System.IO.Path]::GetTempPath()) ("pfl-seed-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
if ($Source) {
    # Reuse an existing verified profile database (for example one produced by a
    # live run) instead of generating a new project.
    if (-not (Test-Path $Source)) { throw "source database not found: $Source" }
    Write-Host "Using existing database $Source" -ForegroundColor Cyan
    $sourceDb = $Source
} else {
    Write-Host "Staging a synthetic project in $staging" -ForegroundColor Cyan
    if ($Live) {
        & $cli live --workspace $staging --endpoint $Endpoint --model $Model
    } else {
        & $cli pipeline --workspace $staging
    }
    if ($LASTEXITCODE -ne 0) { throw "seeding failed (exit $LASTEXITCODE)" }
    $sourceDb = Join-Path $staging 'patent-forge-lite.sqlite'
}

New-Item -ItemType Directory -Force -Path $data | Out-Null
if (Test-Path $db) { Remove-Item $db -Force }
# Copy only the database: never copy the CLI's workspace lock file.
Copy-Item $sourceDb $db -Force
if (Test-Path (Join-Path $data 'projects.lock')) { Remove-Item (Join-Path $data 'projects.lock') -Force }

Write-Host "`nVerifying the seeded profile with pfl-cli inspect ..." -ForegroundColor Cyan
$json = & $cli inspect --database $db
if ($LASTEXITCODE -ne 0) { throw "the seeded database could not be read back" }
$summary = $json | ConvertFrom-Json
Write-Host ("  project : {0}" -f $summary.projects[0].title)
Write-Host ("  runs    : {0}" -f @($summary.projects[0].runs).Count)
foreach ($run in $summary.projects[0].runs) {
    Write-Host ("    v{0} {1,-12} {2,-10} {3} checkpoints, {4} report chars" -f `
        $run.version, $run.workflow, $run.outcome, $run.checkpoints, $run.report_chars)
}
Write-Host "`nSeeded: $db" -ForegroundColor Green
Write-Host "Launch: & '$root\target\debug\pfl-desktop.exe' --replay"
Write-Host "`nTo reset to a clean first-run profile later: Remove-Item '$data' -Recurse -Force"
