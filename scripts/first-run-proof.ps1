# Phase D acceptance: the first-run profile prompt.
#
# A fresh profile asks Full or Lean before anything else. Choosing records the choice,
# and the two profiles behave as designed: Lean refuses a local runtime, Full accepts it.
# Evidence (JSON + screenshot) goes to docs/evidence/first-run/<stamp>/.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class FrMouse{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class FrFront{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

$probe = (& $Exe --page __probe__ 2>&1 | Out-String)
if ($probe -match 'requires a native build') { throw 'the binary is the non-native stub; rebuild with --features native first' }

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-firstrun-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\first-run' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
if (-not $Port) {
    $socket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $socket.Start(); $Port = ([System.Net.IPEndPoint]$socket.LocalEndpoint).Port; $socket.Stop()
}

$script:appPid = 0
$script:mockPid = 0
$script:savedLocalAppData = $env:LOCALAPPDATA
$script:results = [ordered]@{}

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:mockPid) { Stop-Process -Id $script:mockPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
}
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:mockPid) { Stop-Process -Id $script:mockPid -Force -ErrorAction SilentlyContinue }
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
    if ($handle) { [FrFront]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 200 }
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
        [FrMouse]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [FrMouse]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 900
}
function Set-Field([string]$id, [string]$value) {
    $field = Wait-ById $id
    if (-not $field) { throw "field missing: $id" }
    Focus-App
    $field.SetFocus(); Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait($value)
    Start-Sleep -Milliseconds 300
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

# A runtime to test Lean's refusal against.
$mode = Join-Path $OutDir 'mode.txt'
$log = Join-Path $OutDir 'requests.log'
$mock = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'mock-ollama.ps1'), '-Port', $Port, '-LogFile', $log, '-ModeFile', $mode) -PassThru
$script:mockPid = $mock.Id
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path "$mode.ready") -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path "$mode.ready")) { throw 'Ollama mock did not start' }

$profile = Join-Path $OutDir 'profile'
New-Item -ItemType Directory -Force -Path $profile | Out-Null
# No seed: this must be a genuinely fresh profile.
$env:LOCALAPPDATA = $profile
$script:appPid = (Start-Process -FilePath $Exe -ArgumentList @('--local-endpoint', "http://127.0.0.1:$Port") -PassThru).Id
if (-not (Wait-Window)) { throw 'the application window did not appear' }

$prompt = [bool](Wait-ById 'first-run-prompt' 12)
Record 'a fresh profile asks for a profile first' $prompt
Record 'both choices are offered' (([bool](Find-ByName 'Use Full (local and Cloud)')) -and ([bool](Find-ByName 'Use Lean (Cloud only)')))
Shot (Join-Path $OutDir 'first-run.png')

# Lean refuses a local runtime.
Invoke-Named 'Use Lean (Cloud only)'
Record 'the prompt closes after choosing' (-not [bool](Find-ById 'first-run-prompt'))
Invoke-Named 'Setup page'
Set-Field 'input-Local endpoint' "http://127.0.0.1:$Port"
Invoke-Named 'Configure existing local runtime'
Start-Sleep -Milliseconds 1500
Record 'Lean refuses a local runtime' ((Notice) -like '*Lean*') "(notice: $(Notice))"

# Full accepts the same runtime.
Invoke-Named 'Setup page'
Invoke-Named 'Full — Local and Cloud'
Start-Sleep -Milliseconds 800
Set-Field 'input-Local endpoint' "http://127.0.0.1:$Port"
Set-Field 'input-Local model name' 'mock-model:latest'
Invoke-Named 'Configure existing local runtime'
Start-Sleep -Milliseconds 1500
Record 'Full accepts a local runtime' ((Notice) -like '*Execution mode configured*') "(notice: $(Notice))"

# The choice persists: relaunching does not ask again.
Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 800
$script:appPid = (Start-Process -FilePath $Exe -ArgumentList @('--local-endpoint', "http://127.0.0.1:$Port") -PassThru).Id
$null = Wait-Window
Record 'a chosen profile is not asked for again' (-not [bool](Wait-ById 'first-run-prompt' 6))

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
Shot (Join-Path $keep 'first-run.png')
Copy-Item (Join-Path $OutDir 'first-run.png') $keep -Force
$script:results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $keep 'first-run.json')

Clear-Harness
$failed = @($script:results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($script:results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
