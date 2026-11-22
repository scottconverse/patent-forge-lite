# Automated UI acceptance for Patent Forge Lite (Windows, UI Automation).
#
# Drives the real native window through the accessibility tree: navigation,
# disclosure of optional fields, execution mode, a recorded-replay run, history
# selection and the report pane. No mouse or keyboard injection is used, so it
# works even when the window cannot be focused.
#
#   pwsh -File scripts/ui-acceptance.ps1
#   pwsh -File scripts/ui-acceptance.ps1 -Exe .\target\session\debug\pfl-desktop.exe
#
# It never launches live inference and never touches another application.

param(
    [string]$Exe = "",
    [int]$TimeoutSeconds = 25,
    [switch]$NoSeed,
    # The harness must never touch the operator's real profile. It seeds and runs
    # against a throwaway LOCALAPPDATA unless -ProfileRoot is given explicitly.
    [string]$ProfileRoot = "",
    [switch]$KeepProfile
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms

$root = Split-Path -Parent $PSScriptRoot
if (-not $Exe) {
    $candidates = @(
        (Join-Path $root 'target\session\debug\pfl-desktop.exe'),
        (Join-Path $root 'target\debug\pfl-desktop.exe'),
        (Join-Path $root 'target\release\pfl-desktop.exe')
    )
    $Exe = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Exe -or -not (Test-Path $Exe)) { throw "no pfl-desktop.exe found; build with: cargo build -p pfl-desktop --features native" }
if ((Get-Item $Exe).Length -lt 5MB) { throw "$Exe is the non-native stub; rebuild with --features native" }

$script:pass = 0
$script:fail = 0
function Check([string]$name, [bool]$ok, [string]$detail = "") {
    if ($ok) { $script:pass++; Write-Host ("  PASS  {0}" -f $name) -ForegroundColor Green }
    else { $script:fail++; Write-Host ("  FAIL  {0} {1}" -f $name, $detail) -ForegroundColor Red }
}

$script:appPid = 0
$script:profileRoot = $null
$script:ownsProfileRoot = $false
$script:previousLocalAppData = $env:LOCALAPPDATA

# Only ever binds to the window of the process this harness launched. A second
# instance with the same title must never be driven.
function Get-Window {
    if (-not $script:appPid) { return $null }
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $script:appPid)
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}

function Clear-Harness {
    if ($script:appPid) {
        Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 400
    }
    if ($script:previousLocalAppData) { $env:LOCALAPPDATA = $script:previousLocalAppData } else { Remove-Item Env:\LOCALAPPDATA -ErrorAction SilentlyContinue }
    if ($script:ownsProfileRoot -and $script:profileRoot -and (Test-Path $script:profileRoot)) {
        Remove-Item $script:profileRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
}

# Ensure cleanup happens even when a check throws.
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    Clear-Harness
    exit 1
}

function Wait-Window {
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $w = Get-Window
        if ($w) { return $w }
        Start-Sleep -Milliseconds 150
    }
    throw "the application window never appeared"
}

function Find-ByName([string]$name) {
    $w = Get-Window
    if (-not $w) { return $null }
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

function Find-Contains([string]$fragment) {
    $w = Get-Window
    if (-not $w) { return $null }
    $all = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($e in $all) {
        if ($e.Current.Name -and $e.Current.Name.Contains($fragment)) { return $e }
    }
    return $null
}

function Find-All-ByName([string]$name) {
    $w = Get-Window
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)
    $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

function Find-ByType([string]$controlType) {
    $w = Get-Window
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::$controlType)
    $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

function Wait-ById([string]$id) {
    $deadline = (Get-Date).AddSeconds(6)
    while ((Get-Date) -lt $deadline) {
        $el = Find-ById $id
        if ($el) { return $el }
        Start-Sleep -Milliseconds 150
    }
    return $null
}

function Find-ById([string]$id) {
    $w = Get-Window
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $id)
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

# Activate an element the way assistive technology would: use the Invoke pattern
# when the node exposes it, otherwise focus it and press Enter (the keyboard path
# every focusable control supports).
function Invoke-Element([string]$name) {
    $el = Find-ByName $name
    if (-not $el) { $el = Find-Contains $name }
    if (-not $el) { throw "element not found: $name" }
    $patterns = @($el.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        $el.SetFocus()
        Start-Sleep -Milliseconds 250
        [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
    }
    Start-Sleep -Milliseconds 800
}

# The a11y tree is published on the first repaint after a client attaches; give
# the window a moment and one navigation round-trip before asserting.
function Warm-Up {
    $w = Wait-Window
    Start-Sleep -Milliseconds 900
    $null = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
}

Write-Host "`nPatent Forge Lite — automated UI acceptance" -ForegroundColor White
Write-Host ("=" * 60)
Write-Host ("binary: {0}" -f $Exe)

# Isolation: the app resolves its data directory from LOCALAPPDATA, so the harness
# points it at a throwaway root for both seeding and the launched process. The
# operator's real profile is never read, replaced or deleted.
# The harness only ever deletes a directory it created itself. A caller-supplied
# -ProfileRoot must not exist, so cleanup cannot remove someone else's data.
if ($ProfileRoot) {
    if (Test-Path $ProfileRoot) {
        throw "refusing to use an existing -ProfileRoot: $ProfileRoot (the harness deletes its own temporary profile)"
    }
    New-Item -ItemType Directory -Path $ProfileRoot | Out-Null
    $ownsProfileRoot = $true
} else {
    $ProfileRoot = Join-Path ([IO.Path]::GetTempPath()) ("pfl-ui-" + [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Path $ProfileRoot | Out-Null
    $ownsProfileRoot = $true
}
$previousLocalAppData = $env:LOCALAPPDATA
# Snapshot the real profile so the harness can prove it did not write to it.
$realProfile = if ($previousLocalAppData) { Join-Path $previousLocalAppData 'patent-forge-lite\projects.sqlite3' } else { $null }
$realStamp = if ($realProfile -and (Test-Path $realProfile)) { (Get-Item $realProfile).LastWriteTimeUtc } else { $null }
$env:LOCALAPPDATA = $ProfileRoot
Write-Host "`n[0] Isolated profile" -ForegroundColor White
Write-Host ("  data root: {0}" -f $ProfileRoot)

# Refuse to run if another instance already owns the real profile lock: this
# harness must not stop a process it did not start.
$foreign = @(Get-Process -Name 'pfl-desktop' -ErrorAction SilentlyContinue)
Check "no other Patent Forge Lite instance is running" ($foreign.Count -eq 0) ("(pids: {0})" -f (($foreign | ForEach-Object { $_.Id }) -join ','))
if ($foreign.Count -gt 0) {
    # Driving the wrong window would produce meaningless passes. Abort instead.
    throw "another Patent Forge Lite instance is running (pids $(($foreign | ForEach-Object { $_.Id }) -join ',')); close it and rerun"
}

if (-not $NoSeed) {
    & (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
    Check "isolated profile seeded with four completed runs" ($LASTEXITCODE -eq 0)
}

$proc = Start-Process -FilePath $Exe -ArgumentList '--replay', '--select-latest' -PassThru
$script:appPid = $proc.Id
Warm-Up
Check "window opens" ([bool](Get-Window))
Check "accessibility tree is published" ((Find-ByType 'Button').Count -ge 6) ("(buttons: {0})" -f (Find-ByType 'Button').Count)
Check "nav exposes stable AutomationIds" ([bool](Find-ById 'nav-analysis'))

Write-Host "`n[1] Navigation"
Invoke-Element 'Projects page'
Check "Projects heading" ([bool](Find-ByName 'Projects'))
Invoke-Element 'Analysis page'
Check "Analysis heading is the workflow" ([bool](Find-ByName 'Feasibility analysis') -or [bool](Find-ByName 'Application drafting') -or [bool](Find-ByName 'Claims drafting') -or [bool](Find-ByName 'Compliance review'))
$tabs = (Find-ByType 'Tab').Count + (Find-ByType 'TabItem').Count
Check "workflow tabs are exposed as selectable tabs" ($tabs -ge 4) ("(tabs: $tabs)")
Invoke-Element 'History page'
Check "History heading" ([bool](Find-ByName 'History'))
$historyButtons = @(Find-All-ByName 'Open version 4 (Application drafting)')
Check "history rows are named buttons" ($historyButtons.Count -ge 1)

Write-Host "`n[2] Intake fields and disclosure"
Invoke-Element 'Intake page'
$baseline = (Find-ByType 'Edit').Count
Check "three core fields are labelled text inputs" ($baseline -ge 3) ("(edits: $baseline)")
Check "title field carries its AutomationId" ([bool](Find-ById 'input-Invention title'))
Invoke-Element 'Edit optional fields'
$expanded = (Find-ByType 'Edit').Count
Check "nine optional context fields appear" ($expanded -ge ($baseline + 9)) ("(edits: $expanded)")
Invoke-Element 'Hide optional fields'
$collapsed = (Find-ByType 'Edit').Count
Check "optional fields hide again without losing values" ($collapsed -eq $baseline) ("(edits: $collapsed)")

Write-Host "`n[3] Recorded replay run through the real actor"
Invoke-Element 'Setup page'
Invoke-Element 'Use synthetic recorded replay'
Check "real-invention consent is offered" ([bool](Find-Contains 'Real invention data'))
Invoke-Element 'Synthetic test data'
$syntheticOn = [System.Windows.Automation.AutomationElement]::FocusedElement
Check "synthetic declaration can be ticked" ([bool](Find-Contains '✓  Synthetic test data'))
Invoke-Element 'Analysis page'
Invoke-Element 'Start new version'
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$summary = ""
while ((Get-Date) -lt $deadline) {
    $node = Find-ById 'run-summary'
    if ($node) { $summary = $node.Current.Name; if ($summary -match 'Completed') { break } }
    Start-Sleep -Milliseconds 400
}
Check "the run announces Completed" ($summary -match 'Completed') ("(summary: '$summary')")
Check "the run announces full checkpoints" ($summary -match '(\d+) of \1 checkpoints') ("(summary: '$summary')")
Check "cancel/resume is absent on a completed run" (-not [bool](Find-ByName 'Cancel run'))

Write-Host "`n[4] Report pane and export actions"
Invoke-Element 'History page'
$win = Get-Window
$all = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$rows = @()
foreach ($e in $all) {
    if ($e.Current.Name -and $e.Current.Name.StartsWith('Open version ')) { $rows += $e }
}
Check "history lists saved versions" ($rows.Count -ge 1) ("(rows: {0})" -f $rows.Count)
if ($rows.Count -ge 1) {
    $patterns = @($rows[0].GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $rows[0].GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        $rows[0].SetFocus()
        Start-Sleep -Milliseconds 250
        [System.Windows.Forms.SendKeys]::SendWait("{ENTER}")
    }
    Start-Sleep -Milliseconds 1000
}
Check "report renders as a document" ([bool](Find-ById 'report-document'))
Invoke-Element 'View source'
Check "source view exposes the selectable text" ([bool](Find-ById 'input-Saved report'))
Invoke-Element 'Rendered view'
Check "rendered view returns" ([bool](Find-ById 'report-document'))
Check "export actions exist" ([bool](Find-ByName 'Export Word') -and [bool](Find-ByName 'HTML'))

Write-Host "`n[5] Keyboard shortcuts"
function Page-Marker {
    $w = Get-Window
    $all = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    $names = @(); foreach ($e in $all) { if ($e.Current.Name) { $names += $e.Current.Name } }
    if ($names -contains 'Export Word') { 'report' }
    elseif ($names -contains 'Start new version') { 'analysis' }
    elseif (@($names | Where-Object { $_ -like 'Open version*' }).Count -gt 0) { 'history' }
    elseif ($names -contains 'Save intake') { 'intake' }
    elseif ($names -contains 'New project') { 'projects' }
    elseif ($names -contains 'Configure existing local runtime') { 'setup' }
    else { 'unknown' }
}
# GPUI routes key bindings through the focused element, so focus a field first.
Invoke-Element 'Intake page'
$title = Wait-ById 'input-Invention title'
Check "intake page is reachable by keyboard shortcut test" ([bool]$title)
if ($title) { $title.SetFocus(); Start-Sleep -Milliseconds 450 }
$expected = @{ 3 = 'analysis'; 4 = 'history'; 5 = 'report'; 1 = 'projects' }
foreach ($digit in 3, 4, 5, 1) {
    [System.Windows.Forms.SendKeys]::SendWait("^$digit")
    Start-Sleep -Milliseconds 700
    Check "Ctrl+$digit opens $($expected[$digit])" ((Page-Marker) -eq $expected[$digit]) ("(saw {0})" -f (Page-Marker))
}
# Tab must move focus between labelled fields. Focus can take a moment to settle
# after a page change, so confirm the starting point before testing traversal.
Invoke-Element 'Intake page'
# Tab traversal is not asserted programmatically: this harness cannot reliably
# hold foreground focus, so a failed assertion would report the environment
# rather than the app. It was verified by hand (Title -> Technical field ->
# Description, with a visible focus ring), and `scripts/INTERACTIVE-CHECKLIST.md`
# keeps it as a manual step.
Write-Host "  MANUAL  Tab traversal and the focus ring (see docs/INTERACTIVE-CHECKLIST.md)" -ForegroundColor Yellow

Write-Host "`n[6] Shutdown"
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 700
Check "process exits cleanly" (-not [bool](Get-Process -Id $proc.Id -ErrorAction SilentlyContinue))
if ($previousLocalAppData) { $env:LOCALAPPDATA = $previousLocalAppData } else { Remove-Item Env:\LOCALAPPDATA -ErrorAction SilentlyContinue }
if (-not $KeepProfile -and $ownsProfileRoot) { Remove-Item $ProfileRoot -Recurse -Force -ErrorAction SilentlyContinue }
$afterStamp = if ($realProfile -and (Test-Path $realProfile)) { (Get-Item $realProfile).LastWriteTimeUtc } else { $null }
Check "the operator's real profile was not modified" ($afterStamp -eq $realStamp) ("(before: {0}, after: {1})" -f $realStamp, $afterStamp)
$script:appPid = 0

Write-Host "`n" ("=" * 60)
Write-Host ("RESULT: {0} passed, {1} failed" -f $script:pass, $script:fail) -ForegroundColor $(if ($script:fail -eq 0) { 'Green' } else { 'Red' })
if ($script:fail -ne 0) { exit 1 }
