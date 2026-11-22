# Phase D acceptance: the model picker.
#
# Lists the models a loopback runtime reports, picks one into the model field, and
# checks that an unreachable runtime reads as unreachable rather than as "no models".
# Evidence (JSON + request log + screenshot) goes to docs/evidence/models/<stamp>/.
param(
    [Parameter(Mandatory)] [string] $Exe,
    [int] $Port = 0,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms, System.Drawing
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class ModMouse{[DllImport("user32.dll")]public static extern void mouse_event(int flags,int dx,int dy,int data,IntPtr extra);}' -ErrorAction SilentlyContinue
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class ModFront{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

$probe = (& $Exe --page __probe__ 2>&1 | Out-String)
if ($probe -match 'requires a native build') { throw 'the binary is the non-native stub; rebuild with --features native first' }

if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-models-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\models' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

# A port that was just released, so nothing is listening on it.
$closedSocket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$closedSocket.Start(); $ClosedPort = ([System.Net.IPEndPoint]$closedSocket.LocalEndpoint).Port; $closedSocket.Stop()
$log = Join-Path $OutDir 'requests.log'
$mode = Join-Path $OutDir 'mode.txt'

$script:appPid = 0
$script:mockPid = 0
$script:savedLocalAppData = $env:LOCALAPPDATA

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
    if ($handle) { [ModFront]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 200 }
}
function Scroll([int]$ticks, [int]$direction = -120) {
    $w = Get-Window
    if (-not $w) { return }
    $r = $w.Current.BoundingRectangle
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point ([int]($r.X + $r.Width / 2)), ([int]($r.Y + $r.Height / 2))
    for ($i = 0; $i -lt $ticks; $i++) {
        [ModMouse]::mouse_event(0x0800, 0, 0, $direction, [IntPtr]::Zero)
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
        [ModMouse]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 60
        [ModMouse]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 900
}
function Set-Field([string]$id, [string]$value) {
    $field = Wait-ById $id
    if (-not $field) { throw "field missing: $id" }
    Bring-Into-View $field
    Focus-App
    $field.SetFocus(); Start-Sleep -Milliseconds 300
    [System.Windows.Forms.SendKeys]::SendWait('^a')
    [System.Windows.Forms.SendKeys]::SendWait($value)
    Start-Sleep -Milliseconds 300
}
function Readiness {
    $node = Find-ById 'local-readiness'
    if ($node) { $node.Current.Name } else { '' }
}
function Notice {
    $node = Find-ById 'status-notice'
    if ($node) { $node.Current.Name } else { '' }
}
function Wait-Readiness([string]$fragment, [int]$seconds = 12) {
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ((Readiness) -like "*$fragment*") { return $true }
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

if (-not $Port) {
    $socket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $socket.Start(); $Port = ([System.Net.IPEndPoint]$socket.LocalEndpoint).Port; $socket.Stop()
}
$mock = Start-Process -FilePath 'pwsh' -ArgumentList @('-NoProfile', '-File', (Join-Path $PSScriptRoot 'mock-ollama.ps1'), '-Port', $Port, '-LogFile', $log, '-ModeFile', $mode) -PassThru
$script:mockPid = $mock.Id
$deadline = (Get-Date).AddSeconds(15)
while (-not (Test-Path "$mode.ready") -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path "$mode.ready")) { throw 'Ollama mock did not start' }

$profile = Join-Path $OutDir 'profile'
New-Item -ItemType Directory -Force -Path $profile | Out-Null
$env:LOCALAPPDATA = $profile
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
if (-not $Port) {
    $socket = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $socket.Start(); $Port = ([System.Net.IPEndPoint]$socket.LocalEndpoint).Port; $socket.Stop()
}
$script:appPid = (Start-Process -FilePath $Exe -ArgumentList @('--select-latest', '--local-endpoint', "http://127.0.0.1:$Port") -PassThru).Id
if (-not (Wait-Window)) { throw 'the application window did not appear' }

Invoke-Named 'Setup page'
Invoke-Named 'List installed models'
Record 'the runtime reports its installed models' (Wait-Readiness '2 model(s)') "(readiness: $(Readiness))"

Invoke-Named 'mock-model:latest'
Record 'picking a model fills the model field' ((Notice) -like '*mock-model:latest selected*') "(notice: $(Notice))"

# An unreachable runtime must read as unreachable, not as "no models".
Set-Field 'input-Local endpoint' "http://127.0.0.1:$ClosedPort"
Invoke-Named 'List installed models'
$unreachable = Wait-Readiness 'unreachable' 12
Record 'an unreachable runtime says so' $unreachable "(readiness: $(Readiness))"
Record 'and it does not claim an empty runtime' (-not ((Readiness) -like '*no installed models*')) "(readiness: $(Readiness))"

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
Shot (Join-Path $keep 'models.png')
if (Test-Path $log) { Copy-Item $log (Join-Path $keep 'requests.log') -Force }
$results | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $keep 'models.json')

Clear-Harness
$failed = @($results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }
