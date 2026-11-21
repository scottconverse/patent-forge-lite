; Inno Setup definition for Patent Forge Lite (Windows x64).
;
; NOT BUILT OR SIGNED. Inno Setup is not installed on the development machine, and
; no signing policy has been decided. This file is the packaging source, not
; evidence that an installer exists.
;
; Build (once authorized, with Inno Setup 6 installed):
;   cargo build -p pfl-desktop --features native --release --locked
;   iscc packaging\windows\patent-forge-lite.iss
;
; Signing must follow whatever policy Scott selects (GitHub-hosted SignPath or
; Azure Trusted Signing); nothing here performs or implies a signing decision.

#define AppName "Patent Forge Lite"
#define AppVersion "0.1.0"
#define AppPublisher "Scott Converse"
#define AppId "com.scottconverse.patentforgelite"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\Patent Forge Lite
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputBaseFilename=patent-forge-lite-{#AppVersion}-x64
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Per-user data is NOT touched by the installer or uninstaller:
;   %LOCALAPPDATA%\patent-forge-lite
UninstallDisplayIcon={app}\pfl-desktop.exe
SetupIconFile=..\..\assets\brand\patentforge.ico
; Signing tool directives are intentionally absent until a policy is chosen.

[Files]
Source: "..\..\target\release\pfl-desktop.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-PROMPTS"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LEGAL_NOTICE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs
Source: "..\..\prompts\*"; DestDir: "{app}\prompts"; Flags: ignoreversion recursesubdirs
Source: "..\..\assets\brand\patentforge.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\fonts\*"; DestDir: "{app}\fonts"; Flags: ignoreversion
Source: "..\..\assets\fonts\OFL.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\pfl-desktop.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\pfl-desktop.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Run]
Filename: "{app}\pfl-desktop.exe"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Only generated shortcuts; the application's data directory is preserved
; deliberately so a reinstall or upgrade keeps history. A user-initiated
; "remove my data" action is a separate product decision.
Type: filesandordirs; Name: "{group}"
