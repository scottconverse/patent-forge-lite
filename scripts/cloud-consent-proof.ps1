# Phase A acceptance: declined versus approved Cloud consent, against a loopback mock.
#
# Built on the helper style proven in ui-acceptance.ps1: actions are resolved by
# name first, then by substring, and only then invoked -- an inner label has no
# InvokePattern, which is what made earlier attempts no-op silently.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class PflWheel{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-cloud-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not $Port) {
    $probe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $probe.Start(); $Port = ([System.Net.IPEndPoint]$probe.LocalEndpoint).Port; $probe.Stop()
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

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    $colour = if ($ok) { 'Green' } else { 'Red' }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $colour
}
function Requests { if (Test-Path $log) { (Get-Content $log | Measure-Object).Count } else { 0 } }

function Get-Window {
    if (-not $script:appPid) { return $null }
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $script:appPid)
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
function Wait-Window {
    $deadline = (Get-Date).AddSeconds(20)
    while ((Get-Date) -lt $deadline) {
        $w = Get-Window
        if ($w) { Start-Sleep -Milliseconds 900; $null = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition); return $w }
        Start-Sleep -Milliseconds 250
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
    foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
        if ($e.Current.Name -and $e.Current.Name.Contains($fragment)) { return $e }
    }
    return $null
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
function Scroll-Content([int]$ticks = 6) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    for ($i = 0; $i -lt $ticks; $i++) {
        [PflWheel]::mouse_event(0x0800, 0, 0, -120, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 120
    }
}
function Scroll-Top([int]$ticks = 8) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    for ($i = 0; $i -lt $ticks; $i++) {
        [PflWheel]::mouse_event(0x0800, 0, 0, 120, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 120
    }
}
# Name first: an inner label has no InvokePattern, so resolving by substring alone
# silently does nothing.
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
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
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
$null = Wait-Window

Invoke-Element 'Setup page'
Scroll-Content 6
Set-Field 'input-Cloud model name' 'claude-mock'
Set-Field 'input-Cloud API key' 'sk-ant-session-only-test'
Invoke-Element 'Configure Cloud with this key'
$deadline = (Get-Date).AddSeconds(10)
while ((Get-Date) -lt $deadline -and (Notice) -notlike '*Execution mode configured*') { Start-Sleep -Milliseconds 250 }
Record 'Cloud configured against the loopback endpoint' ((Notice) -like '*Execution mode configured*') "(notice: $(Notice))"

Invoke-Element 'Synthetic test data'
Record 'synthetic data declared' ([bool](Find-Contains '✓  Synthetic test data'))

# Declined: paid approval deliberately left off.
Invoke-Element 'Analysis page'
Start-Sleep -Milliseconds 700
# The Start control only exists once a workflow tab is selected.
Invoke-Element 'Feasibility analysis'
Invoke-Element 'Start new version'
Start-Sleep -Milliseconds 1800
Record 'declining sends zero requests' ((Requests) -eq 0) "(requests: $(Requests))"
Record 'refusal names the missing paid approval' ((Notice) -like '*paid Cloud*') "(notice: $(Notice))"

# Approved: grant the per-run approval, then start.
Invoke-Element 'Setup page'
Scroll-Content 6
Invoke-Element '○  Approve paid Cloud request (this run)'
Record 'paid approval recorded' ([bool](Find-Contains '✓  Approve paid Cloud request'))
Invoke-Element 'Analysis page'
Start-Sleep -Milliseconds 700
Invoke-Element 'Feasibility analysis'
Invoke-Element 'Start new version'
Start-Sleep -Milliseconds 900
$deadline = (Get-Date).AddSeconds(25)
while ((Get-Date) -lt $deadline -and (Requests) -eq 0) { Start-Sleep -Milliseconds 300 }
Record 'approved Cloud run reaches the provider' ((Requests) -gt 0) "(requests: $(Requests))"

$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutDir 'cloud-consent.json')
Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence: $OutDir"
if ($failed -gt 0) { exit 1 }
