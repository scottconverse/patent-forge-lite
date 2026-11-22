# Phase B acceptance: editing in the installed app.
#
# Loads a completed claims run into the editor, saves two revisions, and confirms
# both are listed. Evidence (JSON + screenshot) is written under
# docs/evidence/editing/<timestamp>/ so the run can be re-checked later.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class PflMouse{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class PflFront{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

# Refuse the non-native stub: it exits at once and would look like an automation failure.
$probe = (& $Exe --page __probe__ 2>&1 | Out-String)
if ($probe -match 'requires a native build') { throw 'the binary is the non-native stub; rebuild with --features native first' }

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-edit-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\editing' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$script:appPid = 0
$script:savedLocalAppData = $env:LOCALAPPDATA

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
}
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
    exit 1
}

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $(if ($ok) { 'Green' } else { 'Red' })
}
function Get-Window {
    if (-not $script:appPid) { return $null }
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $script:appPid)
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
function Wait-Window {
    $deadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $deadline) {
        $w = Get-Window
        if ($w) {
            Start-Sleep -Milliseconds 900
            $null = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
            return $w
        }
        Start-Sleep -Milliseconds 300
    }
    return $null
}
function Find-ById([string]$id) {
    $w = Get-Window
    if (-not $w) { return $null }
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $id)))
}
function Find-ByName([string]$name) {
    $w = Get-Window
    if (-not $w) { return $null }
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)))
}
function Find-Contains([string]$fragment) {
    $w = Get-Window
    if (-not $w) { return $null }
    $fallback = $null
    foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
        if (-not ($e.Current.Name -and $e.Current.Name.Contains($fragment))) { continue }
        if ($e.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button) { return $e }
        if (-not $fallback) { $fallback = $e }
    }
    return $fallback
}
function Wait-ById([string]$id) {
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        $el = Find-ById $id
        if ($el) { return $el }
        Start-Sleep -Milliseconds 150
    }
    return $null
}
function Focus-App {
    $handle = (Get-Process -Id $script:appPid -ErrorAction SilentlyContinue).MainWindowHandle
    if ($handle) { [PflFront]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 200 }
}
function Scroll([int]$ticks, [int]$direction = -120) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    for ($i = 0; $i -lt $ticks; $i++) {
        [PflMouse]::mouse_event(0x0800, 0, 0, $direction, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 130
    }
}
function Bring-Into-View($element) {
    for ($attempt = 0; $attempt -lt 12; $attempt++) {
        $win = Get-Window
        if (-not $win) { return }
        $w = $win.Current.BoundingRectangle
        $e = $element.Current.BoundingRectangle
        if ($e.Top -ge ($w.Top + 70) -and $e.Bottom -le $w.Bottom) { return }
        if ($e.Top -lt ($w.Top + 70)) { Scroll 2 120 } else { Scroll 2 -120 }
    }
}
function Invoke-Named([string]$name) {
    $el = Find-ByName $name
    if (-not $el) { $el = Find-Contains $name }
    if (-not $el) { throw "element not found: $name" }
    Bring-Into-View $el
    $patterns = @($el.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } elseif ($patterns -contains 'SelectionItemPatternIdentifiers.Pattern') {
        $el.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
    } else {
        Focus-App
        $r = $el.Current.BoundingRectangle
        [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
        Start-Sleep -Milliseconds 150
        [PflMouse]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [PflMouse]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 900
}
function Notice {
    $node = Find-ById 'status-notice'
    if ($node) { $node.Current.Name } else { '' }
}
function Wait-Notice([string]$fragment, [int]$seconds = 12) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ((Notice) -like "*$fragment*") { return $true }
        Start-Sleep -Milliseconds 250
    }
    return $false
}
function Shot([string]$path) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    $bmp = New-Object System.Drawing.Bitmap ([int]$r.Width), ([int]$r.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, (New-Object System.Drawing.Size ([int]$r.Width), ([int]$r.Height)))
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

$profile = Join-Path $OutDir 'profile'
New-Item -ItemType Directory -Force -Path $profile | Out-Null
$env:LOCALAPPDATA = $profile
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
$script:appPid = (Start-Process -FilePath $Exe -ArgumentList '--select-latest' -PassThru).Id
if (-not (Wait-Window)) { throw 'the application window did not appear' }

Invoke-Named 'Analysis page'
Invoke-Named 'Claims'
Invoke-Named 'Load run text'
Record 'run text loads into the editor' ((Notice) -like '*Loaded the run''s final stage*') "(notice: $(Notice))"

Invoke-Named 'Save revision'
Record 'revision 1 saved' (Wait-Notice 'Saved revision 1 of claims document') "(notice: $(Notice))"

# Edit the text: focus the field, select all, type a replacement.
$field = Wait-ById 'input-Revision text'
if (-not $field) { throw 'revision field not found' }
Bring-Into-View $field
Focus-App
$field.SetFocus(); Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('^a')
Start-Sleep -Milliseconds 150
[System.Windows.Forms.SendKeys]::SendWait('1. An edited claim saved from the window.')
Start-Sleep -Milliseconds 500
Invoke-Named 'Save revision'
Record 'revision 2 saved' (Wait-Notice 'Saved revision 2 of claims document') "(notice: $(Notice))"

Invoke-Named 'List revisions'
# The revision rows are plain divs and are not exposed to automation, so the count
# is asserted through the announcement and the screenshot carries the rows.
$listed = Wait-Notice '2 stored revision(s)' 12
Record 'both revisions are listed (2 stored)' $listed "(notice: $(Notice))"

Invoke-Named 'Compare v2 with v1'
Record 'revision comparison reports the change' (Wait-Notice 'Diff v1 to v2') "(notice: $(Notice))"

# Select v2 for compliance and start the review against it.
Invoke-Named 'Use v2 for compliance'
Record 'compliance selection recorded' (Wait-Notice 'Compliance will review claims revision 2') "(notice: $(Notice))"
# A run needs a mode and a data declaration; both live on Setup.
Invoke-Named 'Setup page'
Invoke-Named 'Use synthetic recorded replay'
Invoke-Named 'Synthetic test data'
Record 'replay configured and data declared' ([bool](Find-Contains '✓  Synthetic test data')) "(notice: $(Notice))"
Invoke-Named 'Analysis page'
Invoke-Named 'Compliance review'
Invoke-Named 'Start compliance with selected revisions'
# The run's outcome appears in the summary panel, not the status bar.
$deadline = (Get-Date).AddSeconds(40)
$completed = $false
while ((Get-Date) -lt $deadline) {
    if (Find-Contains 'Completed') { $completed = $true; break }
    Start-Sleep -Milliseconds 300
}
Record 'compliance started with the selection completes' $completed "(notice: $(Notice))"

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
Shot (Join-Path $keep 'editing.png')
$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $keep 'editing.json')

Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
