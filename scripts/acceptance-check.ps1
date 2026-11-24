# Acceptance consistency check.
#
# Makes the matrix self-checking: the summary counts must match the rows, every evidence
# path named in a row must exist, no row may still say a feature is unimplemented while
# another row proves it, and every status must be one the document defines.
param(
    [string] $Acceptance = ""
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Acceptance) { $Acceptance = Join-Path $root 'docs\ACCEPTANCE.md' }
if (-not (Test-Path $Acceptance)) { throw "missing $Acceptance" }

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $(if ($ok) { 'Green' } else { 'Red' })
}

$text = Get-Content $Acceptance -Raw
$lines = Get-Content $Acceptance

# Rows of the phase matrices only: everything from the first phase heading onward, minus
# table separators and header rows.
$start = ($lines | Select-String -Pattern '^## Phase A' | Select-Object -First 1)
if (-not $start) { throw 'no phase matrices found' }
$rows = $lines[$start.LineNumber..($lines.Count - 1)] |
    Where-Object { $_ -match '^\| ' -and $_ -notmatch '^\| ---' -and $_ -notmatch '^\| (Item|Defect) \|' }

function Status-Of([string]$row) {
    if ($row -match '\|\s*pass( —)?') { return 'pass' }
    if ($row -match 'not verified') { return 'not verified' }
    if ($row -match 'blocked — hardware') { return 'blocked — hardware' }
    if ($row -match 'blocked — harness') { return 'blocked — harness' }
    if ($row -match 'blocked — key') { return 'blocked — key' }
    if ($row -match 'blocked — authorization') { return 'blocked — authorization' }
    if ($row -match '\*\*open\*\*') { return 'open defect' }
    return 'unknown'
}

$counts = @{}
foreach ($row in $rows) {
    $status = Status-Of $row
    if (-not $counts.ContainsKey($status)) { $counts[$status] = 0 }
    $counts[$status] = $counts[$status] + 1
}
$unknown = @($rows | Where-Object { (Status-Of $_) -eq 'unknown' })
Record 'every matrix row has a defined status' ($unknown.Count -eq 0) "(unknown: $($unknown.Count))"

# Summary counts must match the rows exactly.
$summaryOk = $true
$summaryDetail = @()
foreach ($status in @('pass', 'not verified', 'blocked — harness', 'blocked — hardware', 'blocked — key', 'blocked — authorization', 'open defect')) {
    $declared = $null
    foreach ($line in $lines) {
        if ($line -match ("^\| " + [regex]::Escape($status) + " \| (\d+) \|")) {
            $declared = [int]$Matches[1]
            break
        }
    }
    $actual = if ($counts.ContainsKey($status)) { $counts[$status] } else { 0 }
    if ($declared -ne $actual) {
        $summaryOk = $false
        $summaryDetail += "$status declared $declared, rows $actual"
    }
}
Record 'the summary matches the rows' $summaryOk ($summaryDetail -join '; ')

# Every evidence path named in a row must exist.
$missing = @()
foreach ($match in [regex]::Matches($text, 'docs/evidence/[A-Za-z0-9_\-/]+')) {
    $path = $match.Value.TrimEnd('/')
    $full = Join-Path $root $path
    if (-not (Test-Path $full)) { $missing += $path }
}
$missing = @($missing | Select-Object -Unique)
Record 'every evidence path named exists' ($missing.Count -eq 0) "(missing: $($missing -join ', '))"

# Documents referenced by the matrix must exist too.
$docMissing = @()
foreach ($match in [regex]::Matches($text, 'docs/[A-Za-z0-9_\-]+\.md')) {
    $full = Join-Path $root $match.Value
    if (-not (Test-Path $full)) { $docMissing += $match.Value }
}
$docMissing = @($docMissing | Select-Object -Unique)
Record 'every document named exists' ($docMissing.Count -eq 0) "(missing: $($docMissing -join ', '))"


# --- Revision and hash agreement with the release manifest -------------------------------
$root = Split-Path -Parent $PSScriptRoot
$manifestPath = Join-Path $root 'docs\RELEASE-MANIFEST.md'
$manifest = if (Test-Path $manifestPath) { Get-Content $manifestPath -Raw } else { '' }
$head = ''
try {
    # A polluted git environment (GIT_CONFIG_COUNT without keys) makes every git call fail;
    # clear it for this probe so the revision comparison is real.
    Get-ChildItem Env: | Where-Object { $_.Name -like 'GIT_*' } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
    $previous = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $head = (& git -C $root rev-parse --short HEAD 2>$null | Out-String).Trim()
    $ErrorActionPreference = $previous
} catch { $head = '' }

$header = [regex]::Match($text, '\| Revision \| `([0-9a-f]+)` \|')
$citedRevision = if ($header.Success) { $header.Groups[1].Value } else { '' }
$manifestRevision = [regex]::Match($manifest, '\| Revision \| `([0-9a-f]+)` \|')
$manifestRev = if ($manifestRevision.Success) { $manifestRevision.Groups[1].Value } else { '' }
Record 'the matrix names a revision the manifest knows' (
    [bool]$citedRevision -and ($citedRevision -eq $head -or $citedRevision -eq $manifestRev)
) "(matrix: $citedRevision, head: $head, manifest: $manifestRev)"

$citedHashes = @([regex]::Matches($text, '\b[0-9a-f]{64}\b') | ForEach-Object { $_.Value } | Select-Object -Unique)
$unknownHashes = @($citedHashes | Where-Object { $manifest -notmatch [regex]::Escape($_) })
Record 'every hash the matrix cites is in the manifest' ($unknownHashes.Count -eq 0) "(unknown: $($unknownHashes -join ', '))"

# --- HANDOFF must point at the summary, not restate it ------------------------------------
$handoffPath = Join-Path $root 'docs\HANDOFF.md'
if (Test-Path $handoffPath) {
    $handoff = Get-Content $handoffPath -Raw
    $restated = [regex]::Match($handoff, '(\d+)\s+pass')
    # A restated count is exactly how HANDOFF fell behind before; it must point instead.
    $handoffDetail = ''
    if ($restated.Success) { $handoffDetail = "(found: $($restated.Groups[0].Value))" }
    Record 'HANDOFF does not restate the pass count' (-not $restated.Success) $handoffDetail
}

# --- Stale claims are scanned in every document a reader meets ----------------------------
$scan = @('docs\ACCEPTANCE.md', 'docs\HANDOFF.md', 'docs\USER-MANUAL.md', 'docs\STATUS.md', 'docs\RELEASE-NOTES.md', 'README.md')
$staleHits = @()
foreach ($relative in $scan) {
    $path = Join-Path $root $relative
    if (-not (Test-Path $path)) { continue }
    $body = Get-Content $path -Raw
    foreach ($phrase in @('not implemented', 'not selectable', 'not tested yet')) {
        if ($body -match [regex]::Escape($phrase)) { $staleHits += "$relative : $phrase" }
    }
}
Record 'no document says a proven feature is unimplemented' ($staleHits.Count -eq 0) "(found: $($staleHits -join '; '))"
# Stale language: a row may not call something unimplemented if a pass row proves it.
$stale = @()
foreach ($phrase in @('not implemented', 'not tested', 'not selectable')) {
    if ($text -match [regex]::Escape($phrase)) { $stale += $phrase }
}
Record 'no row still says a proven feature is unimplemented' ($stale.Count -eq 0) "(found: $($stale -join ', '))"

# Suffix-only wording: the document must not promise single-stage regeneration anywhere.
$singleStage = ($text -match 'single[- ]stage' -and $text -notmatch 'never promises|must not be implemented|no single-stage')
Record 'the document never promises single-stage regeneration' (-not $singleStage)

Write-Host ''
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("RESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
if ($failed -gt 0) { exit 1 }
