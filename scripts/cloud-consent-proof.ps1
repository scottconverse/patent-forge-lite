# Phase A acceptance: declined versus approved Cloud consent, against a loopback mock.
#
# Lessons baked in from earlier attempts:
#   * a workspace build can replace the native binary with a stub that exits, so the
#     binary is probed first;
#   * nav items expose no InvokePattern, so unresolvable controls are clicked at their
#     centre with the window foregrounded;
#   * a control under the page header cannot be clicked reliably, so the data
#     declaration is set before scrolling.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = "",
    # Evidence is written here. Point it outside the repository for a throwaway run; an
    # undeclared parameter meant every run wrote into the tracked tree.
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class PflInput{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class PflFocus{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

# Seven checks called `Record` and the function defined it zero times, so the script threw at the
# first one. It now feeds the file's own `$results` table, which is what the evidence writer reads
# and what the closing summary counts -- a parallel array left the JSON empty.
function Record {
    param([string] $Label, [bool] $Ok, [string] $Detail = '')
    $script:results[$Label] = [pscustomobject]@{ pass = [bool] $Ok; detail = $Detail }
    $tag = if ($Ok) { 'PASS' } else { 'FAIL' }
    Write-Host ("  {0}  {1}{2}" -f $tag, $Label, $(if ($Detail) { " $Detail" } else { '' }))
}

# Refuse to run against the non-native stub: it exits immediately, which would look
# like an automation failure. An invalid page makes either binary exit at once.
$probe = (& $Exe --page __probe__ 2>&1 | Out-String)
if ($probe -match 'requires a native build') {
    throw 'the binary is the non-native stub; rebuild with --features native first'
}

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-cloud-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\cloud-consent' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not $Port) {
    $probeSocket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $probeSocket.Start(); $Port = ([System.Net.IPEndPoint]$probeSocket.LocalEndpoint).Port; $probeSocket.Stop()
}
$log = Join-Path $OutDir 'requests.log'
$profile = Join-Path $OutDir 'profile'
New-Item -ItemType Directory -Force -Path $profile | Out-Null

$script:appPid = 0
$script:mockPid = 0
$script:savedLocalAppData = $env:LOCALAPPDATA
$script:savedCloud = $env:PFL_CLOUD_ENDPOINT

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:mockPid) { Stop-Process -Id $script:mockPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
    if ($script:savedCloud) { $env:PFL_CLOUD_ENDPOINT = $script:savedCloud } else { Remove-Item Env:\PFL_CLOUD_ENDPOINT -ErrorAction SilentlyContinue }
}
trap { Write-Host "ABORTED: $($_.Exception.Message)" -ForegroundColor Red; Clear-Harness; exit 1 }

# Replaces machine-specific absolute paths with placeholders.
#
# Evidence is compiled into the package documentation, so a recorded user-profile path is a
# path leak in every shipped copy. Details stay readable; the operator's name does not travel.
function Sanitise([string]$text) {
    $text = $text.Replace($env:USERPROFILE, '<profile>')
    $text = $text.Replace($env:LOCALAPPDATA, '<localappdata>')
    $text = $text.Replace($OutDir, '<workdir>')
    $text
}

$results = [ordered]@{}

function Requests { if (Test-Path $log) { (Get-Content $log | Measure-Object).Count } else { 0 } }
function MainWindow {
    if (-not $script:appPid) { return $null }
    (Get-Process -Id $script:appPid -ErrorAction SilentlyContinue).MainWindowHandle
}
function Focus-App {
    $handle = MainWindow
    if ($handle) { [PflFocus]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 200 }
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
        if ($e.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -or $e.Current.ControlType -eq [System.Windows.Automation.ControlType]::TabItem) { return $e }
        if (-not $fallback) { $fallback = $e }
    }
    return $fallback
}
function Wait-ById([string]$id) {
    $deadline = (Get-Date).AddSeconds(8)
    while ((Get-Date) -lt $deadline) {
        $el = Find-ById $id
        if ($el) { return $el }
        Start-Sleep -Milliseconds 150
    }
    return $null
}
function Scroll([int]$ticks, [int]$direction = -120) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    # Point at the content column, not the window centre: the right-hand panel does not
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width * 0.4)), ([int]($r.Y + $r.Height * 0.6))
    for ($i = 0; $i -lt $ticks; $i++) {
        [PflInput]::mouse_event(0x0800, 0, 0, $direction, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 130
    }
}
function Bring-Into-View($element) {
    for ($attempt = 0; $attempt -lt 40; $attempt++) {
        $win = Get-Window
        if (-not $win) { return }
        $w = $win.Current.BoundingRectangle
        $e = $element.Current.BoundingRectangle
        if ($e.Top -ge ($w.Top + 70) -and $e.Bottom -le $w.Bottom) { return }
        if ($e.Top -lt ($w.Top + 70)) { Scroll 2 120 } else { Scroll 2 -120 }
    }
}
function Click-Element($element) {
    if (-not $element) {
        $w = Get-Window
        $names = @()
        if ($w) {
            foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
                if ($e.Current.Name -and $e.Current.Name -like '*ynthetic*') { $names += $e.Current.Name }
            }
        }
        throw ("element missing; synthetic-named controls: " + ($names -join ' | '))
    }
    Bring-Into-View $element
    Focus-App
    $r = $element.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    Start-Sleep -Milliseconds 150
    [PflInput]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [PflInput]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
}
function Invoke-Named([string]$name) {
    # Controls before label text: an exact-name lookup can return the text node
    # inside a control, which exposes no action.
    $el = Find-Contains $name
    if (-not $el) { $el = Find-ByName $name }
    if (-not $el) { throw "element not found: $name" }
    # An element outside the window ignores Invoke as well as a click.
    Bring-Into-View $el
    $patterns = @($el.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'SelectionItemPatternIdentifiers.Pattern') {
        # Navigation is a tab list now: selecting an item is the action, and it needs no
        # pointer, so the operator's cursor stays where they left it.
        $el.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
        Start-Sleep -Milliseconds 900
    } elseif ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        Start-Sleep -Milliseconds 800
    } else {
        Click-Element $el
    }
}
# A pointer click on a named control. Needed after typing: the text field keeps focus,
# and an Invoke on a button while a field has focus is swallowed.
# Pointer clicks on some controls are swallowed in this window state; the app's own
# keyboard paths are deterministic, so navigation and the run both use them.
function Nav-Key([string]$keys) {
    Focus-App
    Start-Sleep -Milliseconds 250
    [System.Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds 900
}

# Retry the start: the button is re-resolved on each attempt because the page may
# re-render between selecting the workflow and pressing it.
# Mirror the working Local proof: select the feasibility run, then act on the Analysis
# page. One proven path beats two divergent ones.
function Select-Feasibility-Run {
    Invoke-Named 'History page'
    Start-Sleep -Milliseconds 900
    $row = Find-Contains '(Feasibility analysis)'
    if ($row) {
        $patterns = @($row.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
        if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
            $row.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        } else {
            Click-Element $row
        }
        Start-Sleep -Milliseconds 900
    }
    Invoke-Named 'Analysis page'
    Start-Sleep -Milliseconds 900
}
function Start-Until([string]$expected) {
    $seen = $false
    for ($attempt = 0; $attempt -lt 4; $attempt++) {
        $button = Find-Contains 'Start new version'
        if (-not $button) { $button = Find-ByName 'Start new version' }
        if ($button) {
            $seen = $true
            Bring-Into-View $button
            $patterns = @($button.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
            if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
                $button.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
            } else {
                Click-Element $button
            }
        }
        for ($i = 0; $i -lt 6; $i++) {
            Start-Sleep -Milliseconds 500
            if ((Notice) -like "*$expected*") { return }
        }
    }
    throw ("Start did not take effect; button seen: {0}; notice: {1}" -f $seen, (Notice))
}
function Start-Key {
    Focus-App
    Start-Sleep -Milliseconds 250
    [System.Windows.Forms.SendKeys]::SendWait('^{ENTER}')
    Start-Sleep -Seconds 2
}

function Click-Named([string]$name) {
    $el = Find-ByName $name
    if (-not $el) { $el = Find-Contains $name }
    if (-not $el) { throw "element not found: $name" }
    Bring-Into-View $el
    Focus-App
    $r = $el.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    Start-Sleep -Milliseconds 150
    [PflInput]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [PflInput]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 900
}

function Set-Field([string]$id, [string]$value) {
    $field = Wait-ById $id
    if (-not $field) { throw "field missing: $id" }
    $field.SetFocus(); Start-Sleep -Milliseconds 350
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait($value)
    Start-Sleep -Milliseconds 350
}
function Notice {
    $node = Find-ById 'status-notice'
    if ($node) { $node.Current.Name } else { '' }
}

$mock = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'mock-cloud.ps1'), '-Port', $Port, '-LogFile', $log) -PassThru
$script:mockPid = $mock.Id
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path "$log.ready") -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path "$log.ready")) { throw 'cloud mock did not start' }

$env:LOCALAPPDATA = $profile
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
$env:PFL_CLOUD_ENDPOINT = "http://127.0.0.1:$Port"
$script:appPid = (Start-Process -FilePath $Exe -ArgumentList '--select-latest' -PassThru).Id
if (-not (Wait-Window)) { throw 'the application window did not appear' }

Invoke-Named 'Setup page'
    $null = Wait-ById 'input-Cloud model name'

# Declare the data first, while the page is at the top.
Invoke-Named 'Synthetic test data'
Record 'synthetic data declared' ([bool](Find-Contains '✓  Synthetic test data')) "(notice: $(Notice))"

# Bring the Cloud card into view, then configure against the loopback endpoint.
Bring-Into-View (Wait-ById 'input-Cloud model name')
Set-Field 'input-Cloud model name' 'claude-mock'
Set-Field 'input-Cloud API key' 'sk-ant-session-only-test'
# With a text field focused the button click can be swallowed: clear focus on neutral
# ground first, then press the control.
Focus-App
$wr = (Get-Window).Current.BoundingRectangle
[System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($wr.X + $wr.Width * 0.4)), ([int]($wr.Y + $wr.Height * 0.12))
Start-Sleep -Milliseconds 150
[PflInput]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 60
[PflInput]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 300
# Keyboard activation: the key field still holds focus, so Tab reaches the button and
# Enter presses it. This path does not depend on pointer hit-testing at all.
Focus-App
Start-Sleep -Milliseconds 200
[System.Windows.Forms.SendKeys]::SendWait('{TAB}')
Start-Sleep -Milliseconds 250
[System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
Start-Sleep -Milliseconds 1500
Start-Sleep -Milliseconds 1500
if ((Notice) -notlike '*Execution mode configured*') {
    Invoke-Named 'Configure Cloud with this key'
}
$deadline = (Get-Date).AddSeconds(12)
while ((Get-Date) -lt $deadline -and (Notice) -notlike '*Execution mode configured*') { Start-Sleep -Milliseconds 250 }
Record 'Cloud configured against the loopback endpoint' ((Notice) -like '*Execution mode configured*') "(notice: $(Notice))"

# The confirmation line must name the model and the rates that will apply, while
# stating the total is unknown until usage arrives.
$cost = Wait-ById 'cost-confirmation' 8
$costText = if ($cost) { $cost.Current.Name } else { '' }
Record 'the cost line names the model and the rates' (($costText -like '*claude-mock*') -or ($costText -like '*unknown*')) "(cost: $costText)"
Record 'the cost line says the total is unknown' ($costText -like '*unknown*') "(cost: $costText)"

# A model with recorded rates shows the rates it will apply.
Set-Field 'input-Cloud model name' 'claude-sonnet-4'
Start-Sleep -Milliseconds 600
$rated = Wait-ById 'cost-confirmation' 6
$ratedText = if ($rated) { $rated.Current.Name } else { '' }
Record 'a known model shows the rates applied' (($ratedText -like '*3.00 input*') -and ($ratedText -like '*15.00 output*')) "(cost: $ratedText)"
Set-Field 'input-Cloud model name' 'claude-mock'
Start-Sleep -Milliseconds 400

# Declined: the paid approval is deliberately left off.
Select-Feasibility-Run
Start-Until "paid Cloud"
Start-Sleep -Milliseconds 1500

# Approved: grant the per-run approval, then start again.
Invoke-Named 'Setup page'
$null = Wait-ById 'input-Cloud model name'
Start-Sleep -Milliseconds 600
Invoke-Named 'Approve paid Cloud request'
Record 'paid approval recorded' ([bool](Find-Contains '✓  Approve paid Cloud request')) "(notice: $(Notice))"
Invoke-Named 'Analysis page'
Start-Sleep -Milliseconds 700
$runCost = Wait-ById 'cost-confirmation' 6
$runCostText = if ($runCost) { $runCost.Current.Name } else { '' }
Record 'the cost line is shown before the paid run' (($runCostText -like '*unknown*') -or ($runCostText -like '*per 1M tokens*')) "(cost: $runCostText)"
Select-Feasibility-Run
Start-Until "paid Cloud"
$deadline = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $deadline -and (Requests) -eq 0) { Start-Sleep -Milliseconds 300 }

$results | ConvertTo-Json -Depth 5 | ForEach-Object { Sanitise $_ } | Set-Content (Join-Path $OutDir 'cloud-consent.json')
if ($EvidenceDir) {
    New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
    $stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
    $keep = Join-Path $EvidenceDir $stamp
    New-Item -ItemType Directory -Force -Path $keep | Out-Null
    Copy-Item (Join-Path $OutDir 'cloud-consent.json') $keep -Force
    if (Test-Path $log) { Copy-Item $log $keep -Force }
    Write-Host "evidence kept: $keep"
}
Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence: $OutDir"
if ($failed -gt 0) { exit 1 }
