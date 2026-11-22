# Installed acceptance for the Local path, driven through the real UI.
#
# Proves, against the installed binary and an isolated profile:
#   1. a real-invention declaration starts a Local run and reaches Completed;
#   2. a stream without a terminal frame is displayed as Truncated, not Completed;
#   3. resuming that same run finishes it without repeating completed stages.
#
# The provider is a loopback mock (scripts/mock-ollama.ps1). No model is contacted,
# no paid provider is used, and the operator's profile is never touched.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing

$root = Split-Path -Parent $PSScriptRoot
if (-not $OutDir) { $OutDir = Join-Path ([IO.Path]::GetTempPath()) ("pfl-local-proof-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not $Port) {
    $probe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $probe.Start(); $Port = ([System.Net.IPEndPoint]$probe.LocalEndpoint).Port; $probe.Stop()
}

$profileRoot = Join-Path $OutDir 'profile'
New-Item -ItemType Directory -Force -Path $profileRoot | Out-Null
$modeFile = Join-Path $OutDir 'mode.txt'
$logFile = Join-Path $OutDir 'requests.log'
Set-Content -Path $modeFile -Value 'complete' -NoNewline

$script:appPid = 0
$script:mockPid = 0
$script:previousLocalAppData = $env:LOCALAPPDATA

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:mockPid) { Stop-Process -Id $script:mockPid -Force -ErrorAction SilentlyContinue }
    if ($script:previousLocalAppData) { $env:LOCALAPPDATA = $script:previousLocalAppData }
    Get-Process -Name 'pfl-desktop' -ErrorAction SilentlyContinue | Where-Object { $_.Id -eq $script:appPid } | ForEach-Object { Stop-Process -Id $_.Id -Force }
}
trap { Write-Host "ABORTED: $($_.Exception.Message)" -ForegroundColor Red; Clear-Harness; exit 1 }

$results = [ordered]@{}
function Record([string]$name, [bool]$ok, [string]$detail = '') {
    $results[$name] = [ordered]@{ pass = $ok; detail = $detail }
    $colour = if ($ok) { 'Green' } else { 'Red' }
    Write-Host ("  {0}  {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail) -ForegroundColor $colour
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
function Find-ByName([string]$name) {
    $w = Get-Window; if (-not $w) { return $null }
    $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)))
}
function Find-Contains([string]$fragment) {
    $w = Get-Window; if (-not $w) { return $null }
    foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) {
        if ($e.Current.Name -and $e.Current.Name.Contains($fragment)) { return $e }
    }
    return $null
}
function Activate($element) {
    if (-not $element) { throw "element missing" }
    $patterns = @($element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
    if ($patterns -contains 'InvokePatternIdentifiers.Pattern') {
        $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    } else {
        $element.SetFocus(); Start-Sleep -Milliseconds 250
        [System.Windows.Forms.SendKeys]::SendWait('{ENTER}')
    }
    Start-Sleep -Milliseconds 700
}
function Click([string]$name) { Activate (Find-Contains $name) }
function Set-Field([string]$id, [string]$value) {
    $field = Find-ById $id
    if (-not $field) { throw "field missing: $id" }
    $field.SetFocus(); Start-Sleep -Milliseconds 350
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    Start-Sleep -Milliseconds 150
    [System.Windows.Forms.SendKeys]::SendWait($value)
    Start-Sleep -Milliseconds 400
}
function Run-Summary {
    $node = Find-ById 'run-summary'
    if ($node) { return $node.Current.Name } else { return '' }
}
function Wait-Summary([string]$needle, [int]$seconds = 45) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        $text = Run-Summary
        if ($text -and $text.Contains($needle)) { return $text }
        Start-Sleep -Milliseconds 400
    }
    return (Run-Summary)
}
function Shot([string]$name) {
    $w = Get-Window; if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    $bmp = New-Object System.Drawing.Bitmap ([int]$r.Width), ([int]$r.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, (New-Object System.Drawing.Size ([int]$r.Width), ([int]$r.Height)))
    $bmp.Save((Join-Path $OutDir "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
function Request-Count {
    if (Test-Path $logFile) { (Get-Content $logFile | Measure-Object).Count } else { 0 }
}

# --- start the mock -----------------------------------------------------------------
$mock = Start-Process -FilePath 'pwsh' -ArgumentList @(
    '-NoProfile', '-File', (Join-Path $PSScriptRoot 'mock-ollama.ps1'),
    '-Port', $Port, '-ModeFile', $modeFile, '-LogFile', $logFile) -PassThru
$script:mockPid = $mock.Id
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path "$modeFile.ready") -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path "$modeFile.ready")) { throw 'loopback mock did not start' }
Write-Host "mock on 127.0.0.1:$Port" -ForegroundColor DarkGray

# --- launch the installed app on an isolated profile --------------------------------
$env:LOCALAPPDATA = $profileRoot
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
$app = Start-Process -FilePath $Exe -ArgumentList '--select-latest' -PassThru
$script:appPid = $app.Id
Start-Sleep -Seconds 8
$win = Get-Window
if (-not $win) { throw 'the installed application window did not appear' }
$null = $win.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
Start-Sleep -Milliseconds 800

# --- configure Local against the mock ----------------------------------------------
Activate (Find-ByName 'Setup page')
Start-Sleep -Milliseconds 900
Set-Field 'input-Local endpoint' "http://127.0.0.1:$Port"
Set-Field 'input-Local model name' 'mock-model:latest'
Click 'Configure existing local runtime'
Click 'Full — Local and Cloud'
Click 'Real invention data'
Click 'Authorize local inference'
Record 'real-declared Local mode selected' ([bool](Find-Contains '✓  Real invention data'))

# --- 1. real-declared Local start reaches Completed --------------------------------
# Select the feasibility run so the Analysis page targets feasibility, not the
# application run that happens to be selected at launch.
Activate (Find-ByName 'History page')
Start-Sleep -Milliseconds 800
$row = Find-Contains 'Open version 1 (Feasibility analysis)'
if (-not $row) { $row = Find-Contains '(Feasibility analysis)' }
Activate $row
Start-Sleep -Milliseconds 900
Activate (Find-ByName 'Analysis page')
Start-Sleep -Milliseconds 900
Click 'Start new version'
$summary = Wait-Summary 'Completed'
Record 'real-declared Local run completes' ($summary.Contains('Completed')) "($summary)"
$requestsAfterFirst = Request-Count
Shot 'local-completed'

# --- 2. a truncated stream is displayed as Truncated -------------------------------
# Absolute request index at which to truncate: two stages of this run complete,
# then the stream is cut, so the resume has something real to skip.
$baseline = Request-Count
Set-Content -Path $modeFile -Value ("truncate_after:" + ($baseline + 2)) -NoNewline
Start-Sleep -Milliseconds 300
Click 'Start new version'
$truncated = Wait-Summary 'Truncated'
Record 'truncated stream is displayed as Truncated' ($truncated.Contains('Truncated') -and -not $truncated.Contains('Completed')) "($truncated)"
Shot 'local-truncated'
$requestsBeforeResume = Request-Count
$truncatedRequests = $requestsBeforeResume - $requestsAfterFirst

# --- 3. the same run resumes and does not repeat completed stages ------------------
Set-Content -Path $modeFile -Value 'complete' -NoNewline
Start-Sleep -Milliseconds 300
Click 'Resume this run'
$resumed = Wait-Summary 'Completed'
Record 'the same truncated run resumes to Completed' ($resumed.Contains('Completed')) "($resumed)"
Shot 'local-resumed'
$resumeRequests = (Request-Count) - $requestsBeforeResume
Record 'resume skips stages that already completed' ($resumeRequests -lt $requestsAfterFirst -and $resumeRequests -ge 1) "(first run: $requestsAfterFirst, truncated: $truncatedRequests, resume: $resumeRequests)"

# --- write the record --------------------------------------------------------------
$record = [ordered]@{
    exe             = $Exe
    exe_sha256      = (Get-FileHash $Exe -Algorithm SHA256).Hash.ToLower()
    profile         = $profileRoot
    mock_port       = $Port
    requests_first  = $requestsAfterFirst
    requests_resume = $resumeRequests
    results         = $results
}
$record | ConvertTo-Json -Depth 6 | Set-Content -Path (Join-Path $OutDir 'installed-local-proof.json')
Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence: $OutDir"
if ($failed -gt 0) { exit 1 }
