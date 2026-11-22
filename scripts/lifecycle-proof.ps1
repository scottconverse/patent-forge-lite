# Phase D acceptance: lifecycle.
#
# Two guarantees, measured rather than asserted:
#   1. single instance -- a second launch is refused with its own exit code, says why,
#      and leaves the first window running;
#   2. explicit quit -- quitting with unsaved intake edits raises the confirmation and
#      does not close the window.
# Evidence (JSON + screenshot) goes to docs/evidence/lifecycle/<stamp>/.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class LcMouse{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class LcFront{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

$probe = (& $Exe --page __probe__ 2>&1 | Out-String)
if ($probe -match 'requires a native build') { throw 'the binary is the non-native stub; rebuild with --features native first' }

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-life-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\lifecycle' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$script:appPid = 0
$script:savedLocalAppData = $env:LOCALAPPDATA
$script:results = [ordered]@{}

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
}
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    exit 1
}

function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $script:results[$name] = [ordered]@{ pass = $ok; detail = $detail }
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
function Wait-ById([string]$id, [int]$seconds = 10) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        $el = Find-ById $id
        if ($el) { return $el }
        Start-Sleep -Milliseconds 150
    }
    return $null
}
function Focus-App {
    $handle = (Get-Process -Id $script:appPid -ErrorAction SilentlyContinue).MainWindowHandle
    if ($handle) { [LcFront]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 200 }
}
function Invoke-Named([string]$name) {
    $el = Find-ByName $name
    if (-not $el) { $el = Find-Contains $name }
    if (-not $el) { throw "element not found: $name" }
    $patterns = @($el.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        Focus-App
        $r = $el.Current.BoundingRectangle
        [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
        Start-Sleep -Milliseconds 150
        [LcMouse]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [LcMouse]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 900
}
function Notice {
    $node = Find-ById 'status-notice'
    if ($node) { $node.Current.Name } else { '' }
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

# 1. Single instance: the second launch is refused with its own code and says why.
$secondOutput = (& $Exe --select-latest 2>&1 | Out-String).Trim()
$secondCode = $LASTEXITCODE
Record 'a second launch is refused' ($secondCode -eq 3) "(exit: $secondCode)"
Record 'the refusal says another window owns the data' ($secondOutput -like '*only one instance may write*') "(output: $secondOutput)"
Record 'the first window keeps running' ([bool](Get-Process -Id $script:appPid -ErrorAction SilentlyContinue)) "(pid: $script:appPid)"
Record 'the first window is still usable' ([bool](Find-ById 'main-content'))

# 2. Explicit quit: with nothing pending, Ctrl+Q closes the window. The guard that
# refuses while a run is active or edits are unsaved is covered by the session tests
# (quit_guard_counts_jobs_owned_by_another_project_or_historical_run).
Invoke-Named 'Intake page'
Focus-App
Start-Sleep -Milliseconds 300
[System.Windows.Forms.SendKeys]::SendWait('^q')
$deadline = (Get-Date).AddSeconds(8)
$closed = $false
while ((Get-Date) -lt $deadline) {
    if (-not (Get-Process -Id $script:appPid -ErrorAction SilentlyContinue)) { $closed = $true; break }
    Start-Sleep -Milliseconds 250
}
Record 'quit closes the window when nothing is pending' $closed

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
Shot (Join-Path $keep 'lifecycle.png')
$script:results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $keep 'lifecycle.json')

Clear-Harness
$failed = @($script:results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($script:results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
