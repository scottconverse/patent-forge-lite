# Phase D acceptance: Windows upgrade and rollback.
#
# Installs v0.1.0 into a directory this script owns, puts real content in the isolated
# data location, upgrades to the candidate, then rolls back -- checking after each step
# that the binary changed and the workspace inventory did not. Inno resolves
# {localappdata} from the shell folder rather than the environment, so the install
# directory is always passed explicitly and the test copy is uninstalled at the end.
param(
    [Parameter(Mandatory)] [string] $OldInstaller,
    [Parameter(Mandatory)] [string] $NewInstaller,
    [string] $OutDir = "",
    [string] $EvidenceDir = ""
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition 'using System;using System.Runtime.InteropServices;public class Up2Front{[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr handle);}' -ErrorAction SilentlyContinue

if (-not (Test-Path $OldInstaller)) { throw "missing old installer: $OldInstaller" }
if (-not (Test-Path $NewInstaller)) { throw "missing new installer: $NewInstaller" }
if (-not $OutDir) { $OutDir = Join-Path $env:TEMP ("pfl-upgrade-" + [guid]::NewGuid().ToString('N').Substring(0, 6)) }
if (-not $EvidenceDir) { $EvidenceDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'docs\evidence\upgrade' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$script:savedLocalAppData = $env:LOCALAPPDATA
$script:appPid = 0
$script:launchCount = 0
$script:installDir = Join-Path $OutDir 'app'
$script:results = [ordered]@{}

function Clear-Harness {
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    if ($script:savedLocalAppData) { $env:LOCALAPPDATA = $script:savedLocalAppData }
}
function Remove-TestInstall {
    $uninstaller = Join-Path $script:installDir 'unins000.exe'
    if (Test-Path $uninstaller) {
        Start-Process -FilePath $uninstaller -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -Wait -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 2
    }
}
trap {
    Write-Host ("`nABORTED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    if ($script:appPid) { Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue }
    # A failed proof must not leave a registered test install behind.
    Remove-TestInstall
    exit 1
}

function InstalledExe { Join-Path $script:installDir 'pfl-desktop.exe' }
function DataDir { Join-Path $env:LOCALAPPDATA 'patent-forge-lite' }
function DataFile { Join-Path (DataDir) 'projects.sqlite3' }
function HashOf([string]$path) {
    if (-not (Test-Path $path)) { return '' }
    (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
}
function DataSignature {
    $cli = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\debug\pfl-cli.exe'
    if (-not (Test-Path $cli) -or -not (Test-Path (DataFile))) { return '' }
    (& $cli inspect --database (DataFile) 2>&1 | Out-String).Trim()
}
function Run-Installer([string]$installer) {
    $process = Start-Process -FilePath $installer -ArgumentList @(
        '/VERYSILENT', '/SUPPRESSMSGBOXES', '/CURRENTUSER', '/NORESTART', '/NOCANCEL',
        # No shortcuts and no tasks: this install exists only for the length of the proof.
        '/NOICONS', '/TASKS=""',
        "/DIR=$($script:installDir)"
    ) -PassThru -Wait
    if ($process.ExitCode -ne 0) { throw "installer failed ($($process.ExitCode)): $installer" }
    Start-Sleep -Seconds 2
}
function LaunchAndQuit([string]$exe) {
    $script:lastLaunch = ''
    $script:launchCount = 1 + [int]$script:launchCount
    $errPath = Join-Path $OutDir ("launch-{0}.stderr.txt" -f $script:launchCount)
    $process = Start-Process -FilePath $exe -PassThru -RedirectStandardError $errPath
    $script:appPid = $process.Id
    Start-Sleep -Seconds 8
    $running = [bool](Get-Process -Id $script:appPid -ErrorAction SilentlyContinue)
    if (-not $running) {
        # Say why the build did not stay up: a bare false reads as a harness fault.
        $code = $process.ExitCode
        $tail = ''
        if (Test-Path $errPath) {
            $tail = ((Get-Content $errPath -ErrorAction SilentlyContinue | Select-Object -Last 3) -join ' / ')
        }
        $script:lastLaunch = "exit code $code $tail"
    }
    if ($running) {
        $handle = (Get-Process -Id $script:appPid).MainWindowHandle
        if ($handle) { [Up2Front]::SetForegroundWindow($handle) | Out-Null; Start-Sleep -Milliseconds 300 }
        [System.Windows.Forms.SendKeys]::SendWait('^q')
        Start-Sleep -Seconds 2
    }
    # Prefer a graceful exit, and clear the profile lock if the process had to be killed: a stale
    # lock makes the next build refuse to start, which reads as a broken rollback.
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline -and (Get-Process -Id $script:appPid -ErrorAction SilentlyContinue)) {
        Start-Sleep -Milliseconds 250
    }
    if ([bool](Get-Process -Id $script:appPid -ErrorAction SilentlyContinue)) {
        Stop-Process -Id $script:appPid -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 1
        $lock = (DataFile) + '.lock'
        if (Test-Path $lock) { Remove-Item $lock -Force -ErrorAction SilentlyContinue }
    }
    return $running
}

# Refuse to run where Patent Forge Lite is really installed: these are the product's own
# installers, so registering them would repoint the operator's Start-menu and uninstall entries
# and then remove them, leaving the real product half-uninstalled.
$realKeys = @(
    'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*'
)
$real = @(Get-ItemProperty $realKeys -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -like 'Patent Forge Lite*' })
if ($real.Count -gt 0) {
    throw ("Patent Forge Lite is installed on this machine ({0}); refusing to register and later remove the operator's own install. Run this on a machine without it, or uninstall it first." -f $real[0].DisplayName)
}
Record 'no real install is registered on this machine' $true

$env:LOCALAPPDATA = Join-Path $OutDir 'localappdata'
New-Item -ItemType Directory -Force -Path $env:LOCALAPPDATA | Out-Null

# 1. Install v0.1.0 into the directory this script owns.
Run-Installer $OldInstaller
$oldHash = HashOf (InstalledExe)
Record 'v0.1.0 installs into the test directory' ($oldHash.Length -gt 0) "(sha256: $oldHash)"

# 2. Put real content in the isolated data location first: the CLI refuses a
# non-empty workspace, so the app must not have touched it yet.
# seed-profile writes the application's own database (projects.sqlite3) in the
# redirected profile, so the inventory being checked is the one the app reads.
& (Join-Path $PSScriptRoot 'seed-profile.ps1') -Force | Out-Null
# The v2 column is additive and the schema version is unchanged, so a v1 build can still open a
# workspace written by this build -- which is what makes rollback safe. The proof exercises that
# below by launching the rolled-back build on the migrated data.
$signature = DataSignature
$projects = 0
try { $projects = ([int](($signature | ConvertFrom-Json).project_count)) } catch { $projects = 0 }
Record 'the workspace holds a project and its runs' ($projects -ge 1) "(project_count: $projects)"

# The installed v0.1.0 must open that data, which is what "start on existing data" means.
$ran = LaunchAndQuit (InstalledExe)
Record 'the installed v0.1.0 starts on existing data' $ran ($script:lastLaunch)

# 3. Upgrade.
Run-Installer $NewInstaller
$newHash = HashOf (InstalledExe)
Record 'the upgrade replaces the binary' (($newHash.Length -gt 0) -and ($newHash -ne $oldHash)) "(sha256: $newHash)"
Record 'the workspace inventory survives the upgrade' ((DataSignature) -eq $signature) "(signature unchanged)"
$ran = LaunchAndQuit (InstalledExe)
Record 'the upgraded build starts on the existing data' $ran ($script:lastLaunch)

# 4. Roll back.
Run-Installer $OldInstaller
$backHash = HashOf (InstalledExe)
Record 'the rollback restores the older binary' ($backHash -eq $oldHash) "(sha256: $backHash)"
Record 'the workspace inventory survives the rollback' ((DataSignature) -eq $signature) "(signature unchanged)"
$ran = LaunchAndQuit (InstalledExe)
Record 'the rolled-back build starts on the existing data' $ran ($script:lastLaunch)

# 5. Leave no trace.
Remove-TestInstall
Record 'the test install is removed afterwards' (-not (Test-Path (InstalledExe)))

New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null
$stamp = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
$keep = Join-Path $EvidenceDir $stamp
New-Item -ItemType Directory -Force -Path $keep | Out-Null
$script:results['hashes'] = [ordered]@{
    pass   = $true
    detail = [ordered]@{ old = $oldHash; new = $newHash; rolled_back = $backHash; inventory = $signature }
}
$script:results | ConvertTo-Json -Depth 6 | ForEach-Object { Sanitise $_ } | Set-Content (Join-Path $keep 'upgrade.json')

Clear-Harness
$failed = @($script:results.Values | Where-Object { -not $_.pass }).Count
Write-Host ("`nRESULT: {0} passed, {1} failed" -f ($script:results.Count - $failed), $failed)
Write-Host "evidence kept: $keep"
if ($failed -gt 0) { exit 1 }