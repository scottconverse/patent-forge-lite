# Preflight for an interactive acceptance session.
# Read-only: it inspects the machine and the built binary and prints a verdict.
# It does not launch the desktop, create a profile or contact any provider.
#
#   pwsh -File scripts/preflight.ps1

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'target\session\debug\pfl-desktop.exe'
$exeDefault = Join-Path $root 'target\debug\pfl-desktop.exe'
$release = Join-Path $root 'target\release\pfl-desktop.exe'
$data = Join-Path $env:LOCALAPPDATA 'patent-forge-lite'
$db = Join-Path $data 'projects.sqlite3'
$problems = 0

function Ok([string]$text) { Write-Host "  PASS  $text" -ForegroundColor Green }
function Note([string]$text) { Write-Host "  INFO  $text" -ForegroundColor Cyan }
function Fail([string]$text) { Write-Host "  FAIL  $text" -ForegroundColor Red; $script:problems++ }

Write-Host "`nPatent Forge Lite — interactive session preflight" -ForegroundColor White
Write-Host ("=" * 60)

Write-Host "`n[1] Binary"
$binary = if (Test-Path $exe) { $exe } elseif (Test-Path $exeDefault) { $exeDefault } elseif (Test-Path $release) { $release } else { $null }
if ($binary) {
    $info = Get-Item $binary
    $mib = $info.Length / 1MB
    # A plain `cargo build`/`cargo test` rebuilds the bin target WITHOUT the
    # native feature, overwriting the real binary with a stub that exits 2.
    if ($mib -lt 5) {
        Fail ("{0} is only {1:N2} MiB — this is the non-native stub, not the app." -f $info.Name, $mib)
        Note "Rebuild it: cargo build -p pfl-desktop --features native"
    } else {
        Ok ("{0} ({1:N1} MiB, built {2:yyyy-MM-dd HH:mm}) — native build" -f $info.Name, $mib, $info.LastWriteTime)
    }
} else {
    Fail "no pfl-desktop.exe found; run: cargo build -p pfl-desktop --features native"
}

Write-Host "`n[2] Profile state"
if (Test-Path $db) {
    $size = (Get-Item $db).Length
    Ok ("seeded profile present: {0:N1} MiB" -f ($size / 1MB))
    $lock = Join-Path $data 'projects.lock'
    if (Test-Path $lock) { Note "lock file exists (expected; a stale lock is harmless, it is OS-backed)" }
} else {
    Note "no profile yet — the app will create $db on first launch"
}

Write-Host "`n[3] Running instances"
$running = Get-Process -Name 'pfl-desktop' -ErrorAction SilentlyContinue
if ($running) {
    Fail ("pfl-desktop already running (PID {0}); close it first" -f ($running.Id -join ', '))
} else {
    Ok "no pfl-desktop instance is running (single-owner lock is free)"
}

Write-Host "`n[4] Environment"
$os = Get-CimInstance Win32_OperatingSystem
Ok ("{0} build {1}" -f $os.Caption, $os.BuildNumber)
$gpu = Get-CimInstance Win32_VideoController | Select-Object -First 1
if ($gpu) { Ok ("GPU: {0} driver {1}" -f $gpu.Name, $gpu.DriverVersion) } else { Note "no GPU reported; GPUI needs a working DirectX device" }
$ram = [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 1)
Note ("free RAM: {0} GiB" -f $ram)

Write-Host "`n[5] Optional local inference"
try {
    $tags = Invoke-RestMethod -Uri 'http://127.0.0.1:11434/api/tags' -TimeoutSec 2
    $models = @($tags.models).Count
    Ok ("local Ollama reachable with $models installed models")
} catch {
    Note "local Ollama not reachable (only needed for the live-inference part; replay works without it)"
}

Write-Host ("`n" + ("=" * 60))
if ($problems -eq 0) {
    Write-Host "VERDICT: READY — see docs/INTERACTIVE-CHECKLIST.md" -ForegroundColor Green
    Write-Host "`nLaunch with:"
    Write-Host ("  & '{0}' --replay --select-latest" -f $binary) -ForegroundColor Yellow
    Write-Host "`n(The --replay flag only preselects recorded mode; it does not create or start anything.)"
} else {
    Write-Host "VERDICT: NOT READY — resolve the FAIL lines above first" -ForegroundColor Red
    exit 1
}
