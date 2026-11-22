# Phase A acceptance: declined Cloud consent sends nothing; approved Cloud runs.
#
# Runs against a real application binary (installed build in Phase F, native build
# here), an isolated profile and a loopback Cloud mock. No real Cloud host is
# contacted and no key leaves the machine.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms

$root = Split-Path -Parent $PSScriptRoot
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
$script:previousLocalAppData = $env:LOCALAPPDATA
$script:previousCloudEndpoint = $env:PFL_CLOUD_ENDPOINT

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:mockPid) { Stop-Process -Id $script:mockPid -Force -ErrorAction SilentlyContinue }
    if ($script:previousLocalAppData) { $env:LOCALAPPDATA = $script:previousLocalAppData }
    if ($script:previousCloudEndpoint) { $env:PFL_CLOUD_ENDPOINT = $script:previousCloudEndpoint } else { Remove-Item Env:\PFL_CLOUD_ENDPOINT -ErrorAction SilentlyContinue }
}
trap { Write-Host "ABORTED: $($_.Exception.Message)" -ForegroundColor Red; Clear-Harness; exit 1 }

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    $colour = if ($ok) { 'Green' } else { 'Red' }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $colour
}
function Requests {
    if (Test-Path $log) { (Get-Content $log | Measure-Object).Count } else { 0 }
}
function Get-Window {
    if (-not $script:appPid) { return $null }
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $script:appPid)
    [System.Windows.Automation.AutomationElement]::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}
function Find-ById([string]$id) {
    $w = Get-Window; if (-not $w) { return $null }
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::AutomationIdProperty, $id)))
}
function Find-Contains([string]$fragment) {
    $w = Get-Window; if (-not $w) { return $null }
    foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
        if ($e.Current.Name -and $e.Current.Name.Contains($fragment)) { return $e }
    }
    return $null
}
function Activate($element) {
    if (-not $element) { throw 'element missing' }
    $patterns = @($element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        $element.SetFocus(); Start-Sleep -Milliseconds 250
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
    Start-Sleep -Milliseconds 700
}
function Set-Field([string]$id, [string]$value) {
    $field = Find-ById $id
    if (-not $field) { throw "field missing: $id" }
    $field.SetFocus(); Start-Sleep -Milliseconds 350
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait($value)
    Start-Sleep -Milliseconds 400
}

# --- mock -------------------------------------------------------------------------
$mock = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'mock-cloud.ps1'), '-Port', $Port, '-LogFile', $log) -PassThru
$script:mockPid = $mock.Id
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path "$log.ready") -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path "$log.ready")) { throw 'cloud mock did not start' }

# --- app --------------------------------------------------------------------------
$env:LOCALAPPDATA = $profile
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
$env:PFL_CLOUD_ENDPOINT = "http://127.0.0.1:$Port"
$app = Start-Process -FilePath $Exe -ArgumentList '--select-latest' -PassThru
$script:appPid = $app.Id
Start-Sleep -Seconds 8
$win = Get-Window
if (-not $win) { throw 'window did not appear' }
$null = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
Start-Sleep -Milliseconds 800

Activate (Find-Contains 'Setup page')
Start-Sleep -Milliseconds 500
# The Cloud card sits below the local runtime card, so scroll it into view before
# addressing its fields; unrealised elements are invisible to automation.
$anchor = Find-ById 'input-Local endpoint'
if ($anchor) { $anchor.SetFocus() }
for ($i = 0; $i -lt 5; $i++) {
    [System.Windows.Forms.SendKeys]::SendWait('{PGDN}')
    Start-Sleep -Milliseconds 350
}
Set-Field 'input-Cloud model name' 'claude-mock'
Set-Field 'input-Cloud API key' 'sk-ant-session-only-test'
Activate (Find-Contains 'Configure Cloud with this key')
Record 'Cloud is configured against the loopback endpoint' ([bool](Find-Contains 'Cloud · paid per run'))

# Declined: approval left off, start attempted.
Activate (Find-Contains 'Analysis page')
Start-Sleep -Milliseconds 600
Activate (Find-Contains 'Start new version')
Start-Sleep -Milliseconds 1500
$declined = Requests
Record 'declining sends zero requests' ($declined -eq 0) "(requests: $declined)"
$notice = Find-Contains 'Approve the paid Cloud request'
Record 'the refusal names the missing approval' ([bool]$notice)

# Approved: toggle the per-run approval and start again.
Activate (Find-Contains 'Setup page')
Activate (Find-Contains 'Approve paid Cloud request')
Activate (Find-Contains 'Analysis page')
Start-Sleep -Milliseconds 600
Activate (Find-Contains 'Start new version')
Start-Sleep -Seconds 6
$approved = Requests
Record 'an approved Cloud run reaches the provider' ($approved -gt 0) "(requests: $approved)"

$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutDir 'cloud-consent.json')
Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence: $OutDir"
if ($failed -gt 0) { exit 1 }
