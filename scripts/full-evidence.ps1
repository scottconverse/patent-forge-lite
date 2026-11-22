# Run every proof against one revision and keep one summary.
#
# Order matters: the fast structural checks first, then the CLI proofs, then the
# installed ones. Each script keeps its own timestamped evidence; this adds a single
# summary saying which scripts passed, when, and against which binary.
param(
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $EvidenceDir) { $EvidenceDir = Join-Path $root 'docs\evidence\full' }
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$runDir = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $runDir | Out-Null

$nativeExe = Join-Path $root 'target\native\debug\pfl-desktop.exe'
$installers = @{
    old = Join-Path $root 'dist\patent-forge-lite-0.1.0-x64.exe'
    new = Join-Path $root 'dist\patent-forge-lite-0.2.0-x64.exe'
}

$scripts = [ordered]@{
    'acceptance-check'    = @{ args = @{} }
    'api-proof'           = @{ args = @{} }
    'migration-proof'     = @{ args = @{} }
    'models-proof'        = @{ args = @{ Exe = $nativeExe } }
    'first-run-proof'     = @{ args = @{ Exe = $nativeExe } }
    'lifecycle-proof'     = @{ args = @{ Exe = $nativeExe } }
    'editing-proof'       = @{ args = @{ Exe = $nativeExe } }
    'research-proof'      = @{ args = @{ Exe = $nativeExe } }
    'cloud-consent-proof' = @{ args = @{ Exe = $nativeExe } }
    'installed-local-proof' = @{ args = @{ Exe = $nativeExe } }
    'ui-acceptance'       = @{ args = @{ Exe = $nativeExe } }
    'upgrade-proof'       = @{ args = @{ OldInstaller = $installers.old; NewInstaller = $installers.new } }
}

$summary = [ordered]@{}
foreach ($name in $scripts.Keys) {
    $script = Join-Path $PSScriptRoot "$name.ps1"
    if (-not (Test-Path $script)) {
        $summary[$name] = [ordered]@{ pass = $false; detail = 'script missing' }
        Write-Host ("  MISSING  {0}" -f $name) -ForegroundColor Red
        continue
    }
    $arguments = @('-NoProfile', '-File', $script)
    foreach ($key in $scripts[$name].args.Keys) {
        $arguments += "-$key"
        $arguments += $scripts[$name].args[$key]
    }
    Write-Host ("`n=== {0} ===" -f $name) -ForegroundColor Cyan
    $output = & pwsh @arguments 2>&1 | Out-String
    $code = $LASTEXITCODE
    $resultLine = ($output -split "`r?`n" | Where-Object { $_ -match '^RESULT:' } | Select-Object -Last 1)
    $summary[$name] = [ordered]@{
        pass   = ($code -eq 0)
        detail = if ($resultLine) { $resultLine.Trim() } else { "exit $code" }
    }
    $output | Set-Content (Join-Path $runDir "$name.log")
    $colour = if ($code -eq 0) { 'Green' } else { 'Red' }
    Write-Host ("  {0}  {1} ({2})" -f $(if ($code -eq 0) { 'PASS' } else { 'FAIL' }), $name, $summary[$name].detail) -ForegroundColor $colour
}

$revision = (git -C $root rev-parse --short HEAD).Trim()
$payload = [ordered]@{
    revision = $revision
    when     = $stamp
    binary   = $nativeExe
    scripts  = $summary
}
$payload | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $runDir 'summary.json')

$failed = @($summary.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed (revision {2})" -f ($summary.Count - $failed), $failed, $revision)
Write-Host "evidence kept: $runDir"
if ($failed -gt 0) { exit 1 }
