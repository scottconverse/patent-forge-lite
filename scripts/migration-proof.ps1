# Phase E acceptance: the copy-first migration, on a documented fixture.
#
# Creates the synthetic legacy fixture, previews it, imports it, imports it again, and
# checks the receipt, the resulting workspace, provenance warnings, preserved archive
# records and that the source was never modified. Nothing here reads a real
# installation: the fixture is authored data written to a new path.
param(
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$cli = Join-Path $root 'target\debug\pfl-cli.exe'
if (-not (Test-Path $cli)) { throw "missing $cli - run: cargo build -p pfl-cli" }
if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-migrate-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path $root 'docs\evidence\migration' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $(if ($ok) { 'Green' } else { 'Red' })
}
function Invoke-Cli([string[]]$arguments) {
    $output = (& $cli @arguments 2>&1 | Out-String).Trim()
    [pscustomobject]@{ code = $LASTEXITCODE; text = $output }
}

$fixture = Join-Path $OutDir 'legacy.sqlite'
$made = Invoke-Cli @('legacy-fixture', '--out', $fixture)
Record 'the fixture is created at a new path' (($made.code -eq 0) -and (Test-Path $fixture)) "(exit: $($made.code))"
$sourceHash = (Get-FileHash $fixture -Algorithm SHA256).Hash.ToLower()

# Preview: counts and preserved-but-uninterpreted tables, with no destination written.
$preview = (Invoke-Cli @('legacy-preview', '--source', $fixture)).text | ConvertFrom-Json
Record 'the preview reports the projects and runs' (($preview.projects -eq 1) -and ($preview.runs -eq 2)) "(projects: $($preview.projects), runs: $($preview.runs))"
Record 'the preview surfaces archive-only records' (($preview.archived_records -gt 0) -and ($preview.unknown_tables.Count -gt 0)) "(archived: $($preview.archived_records), tables: $($preview.unknown_tables.Count))"
Record 'the preview states its provenance limits' (($preview.warnings -join ' ') -like '*reconstructed*') "(warnings: $($preview.warnings.Count))"

$workspace = Join-Path $OutDir 'ws'
$first = (Invoke-Cli @('migrate', '--source', $fixture, '--workspace', $workspace)).text | ConvertFrom-Json
Record 'the import reports what it moved' (($first.projects_imported -eq 1) -and ($first.runs_imported -eq 2) -and ($first.records_preserved -gt 0)) "(projects: $($first.projects_imported), runs: $($first.runs_imported), records: $($first.records_preserved))"
Record 'a backup of the source is written' (Test-Path $first.backup_path) "(backup: $($first.backup_path))"
Record 'the import activates a new database' ((Test-Path $first.activated_path) -and (-not $first.repeated))
Record 'the import is not marked repeated on a first run' (-not $first.repeated)

$inspect = (Invoke-Cli @('inspect', '--database', $first.activated_path)).text | ConvertFrom-Json
Record 'the activated workspace holds the imported project' ($inspect.project_count -eq 1) "(projects: $($inspect.project_count))"
$runsImported = @($inspect.projects[0].runs).Count
Record 'both imported runs are present' ($runsImported -eq 2) "(runs: $runsImported)"

# Re-run: same fixture, same workspace.
$second = (Invoke-Cli @('migrate', '--source', $fixture, '--workspace', $workspace)).text | ConvertFrom-Json
Record 'a repeated import is recognised as repeated' ($second.repeated) "(repeated: $($second.repeated))"
$inspectAgain = (Invoke-Cli @('inspect', '--database', $second.activated_path)).text | ConvertFrom-Json
$runsAgain = @($inspectAgain.projects[0].runs).Count
Record 'a repeated import does not duplicate anything' (($inspectAgain.project_count -eq 1) -and ($runsAgain -eq 2)) "(projects: $($inspectAgain.project_count), runs: $runsAgain)"

# No credential may survive into the activated workspace, not even redacted-in-name
# only: the fixture's own secrets must be absent from the file.
$activatedBytes = [System.IO.File]::ReadAllBytes($first.activated_path)
$activatedText = [System.Text.Encoding]::UTF8.GetString($activatedBytes)
$saltAbsent = -not $activatedText.Contains('synthetic-not-a-real-encryption-salt')
$zerosAbsent = -not $activatedText.Contains(('0' * 40))
$onesAbsent = -not $activatedText.Contains(('1' * 40))
Record 'no credential survives into the imported workspace' ($saltAbsent -and $zerosAbsent -and $onesAbsent) "(salt absent: $saltAbsent, zero-key absent: $zerosAbsent, uspto-key absent: $onesAbsent)"

$afterHash = (Get-FileHash $fixture -Algorithm SHA256).Hash.ToLower()
Record 'the source is untouched' ($afterHash -eq $sourceHash) "(sha256: $afterHash)"

# A missing source must be an error, never an empty import.
$missing = Invoke-Cli @('legacy-preview', '--source', (Join-Path $OutDir 'not-there.sqlite'))
Record 'a missing source is an error, not an empty import' (($missing.code -ne 0) -and ($missing.text -like '*no legacy database*')) "(exit: $($missing.code))"

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
$results['receipts'] = [ordered]@{
    pass   = $true
    detail = [ordered]@{ first = $first; second = $second; preview = $preview }
}
$results | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $keep 'migration.json')

$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
