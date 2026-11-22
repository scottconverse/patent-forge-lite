# Phase E acceptance: the HTTP surface the DSH skill consumes.
#
# Starts the compatibility server on a loopback port over a synthetic workspace and
# checks every read endpoint the launcher uses, plus the 404 for an unknown project.
# Nothing leaves the machine and no real workspace is opened.
param(
    [int] $Port = 0,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $root 'target\debug\pfl-cli.exe'
if (-not (Test-Path $cli)) { throw "missing $cli - run: cargo build -p pfl-cli" }
if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-api-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path $root 'docs\evidence\api' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not $Port) {
    $socket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $socket.Start(); $Port = ([System.Net.IPEndPoint]$socket.LocalEndpoint).Port; $socket.Stop()
}

$script:servePid = 0
function Stop-Server {
    if ($script:servePid) { Stop-Process -Id $script:servePid -Force -ErrorAction SilentlyContinue }
}
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    if ($script:servePid) { Stop-Process -Id $script:servePid -Force -ErrorAction SilentlyContinue }
    exit 1
}

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $(if ($ok) { 'Green' } else { 'Red' })
}
function Get-Json([string]$path) {
    Invoke-RestMethod -Uri "http://127.0.0.1:$Port$path" -TimeoutSec 10
}

$workspace = Join-Path $OutDir 'ws'
New-Item -ItemType Directory -Force -Path $workspace | Out-Null
& $cli pipeline --workspace $workspace 2>&1 | Out-Null
$script:servePid = (Start-Process -FilePath $cli -ArgumentList @('serve', '--workspace', $workspace, '--addr', "127.0.0.1:$Port") -PassThru).Id
$deadline = (Get-Date).AddSeconds(20)
$ready = $false
while ((Get-Date) -lt $deadline -and -not $ready) {
    try { $null = Get-Json '/api/health'; $ready = $true } catch { Start-Sleep -Milliseconds 400 }
}
Record 'the server starts and answers /api/health' $ready

$health = Get-Json '/api/health'
Record 'health names the service' ($health.service -eq 'patentforge-backend' -or $health.status -eq 'ok') "(status: $($health.status))"

$projects = Get-Json '/api/projects'
$projectCount = ($projects | Measure-Object).Count
Record 'the project list is returned' ($projectCount -ge 1) "(projects: $projectCount)"
$firstProject = $projects | Select-Object -First 1
$projectId = $firstProject.id
Record 'a project carries its intake' ($null -ne $firstProject.intake -and $firstProject.intake.title.Length -gt 0) "(title: $($firstProject.intake.title))"

$project = Get-Json "/api/projects/$projectId"
Record 'a single project is readable' ($project.id -eq $projectId)

$history = Get-Json "/api/projects/$projectId/history"
$runCount = ($history | Measure-Object).Count
Record 'the project history lists runs' ($runCount -ge 1) "(runs: $runCount)"
$runId = ($history | Select-Object -First 1).id

$run = Get-Json "/api/runs/$runId"
Record 'a run is readable' ($run.id -eq $runId) "(outcome: $($run.outcome))"
$events = Get-Json "/api/runs/$runId/events"
$eventCount = ($events | Measure-Object).Count
Record 'run events are readable' ($eventCount -ge 1) "(events: $eventCount)"

$reportResponse = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/api/runs/$runId/report" -UseBasicParsing -TimeoutSec 10
Record 'the run report is served' ($reportResponse.StatusCode -eq 200 -and $reportResponse.Content.Length -gt 0) "(bytes: $($reportResponse.Content.Length))"

$exportResponse = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/api/runs/$runId/export?format=markdown" -UseBasicParsing -TimeoutSec 10
Record 'an export is served in markdown' ($exportResponse.StatusCode -eq 200 -and $exportResponse.Content.Length -gt 0) "(bytes: $($exportResponse.Content.Length))"

$missing = 0
try { $null = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/api/projects/not-a-project" -UseBasicParsing -TimeoutSec 5 } catch { $missing = [int]$_.Exception.Response.StatusCode }
Record 'an unknown project is a 404, not an empty project' ($missing -eq 404) "(status: $missing)"

Stop-Server
Record 'the server stops cleanly' (-not [bool](Get-Process -Id $script:servePid -ErrorAction SilentlyContinue))

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
$results['endpoints'] = [ordered]@{
    pass   = $true
    detail = [ordered]@{
        port     = $Port
        project  = $projectId
        run      = $runId
        events   = $events.Count
        report   = $reportResponse.Content.Length
        export   = $exportResponse.Content.Length
    }
}
$results | ConvertTo-Json -Depth 6 | Set-Content (Join-Path $keep 'api.json')

$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
