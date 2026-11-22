; Inno Setup definition for Patent Forge Lite (Windows x64).
;
; Build:
;   cargo build -p pfl-desktop --features native --release --locked
;   iscc packaging\windows\patent-forge-lite.iss
;
; Installs per-user (no elevation) into %LOCALAPPDATA%\Programs\PatentForge Lite,
; matching the application's own per-user data model. The profile directory
; %LOCALAPPDATA%\patent-forge-lite is never touched by install or uninstall.
;
; UNSIGNED BY DEFAULT. Signing must follow whichever policy Scott selects
; (GitHub-hosted SignPath or Azure Trusted Signing); this file performs no signing
; and implies no decision. Add SignTool directives only after that choice.

#define AppName "Patent Forge Lite"
; The version can be overridden from the command line (ISCC /DAppVersion=...), so a
; candidate can be built and an upgrade exercised without editing this file.
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#define AppPublisher "Scott Converse"
#define AppId "com.scottconverse.patentforgelite"
#define AppExe "pfl-desktop.exe"

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
VersionInfoVersion={#AppVersion}
VersionInfoDescription={#AppName} installer
VersionInfoProductName={#AppName}
DefaultDirName={localappdata}\Programs\PatentForge Lite
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\..\dist
OutputBaseFilename=patent-forge-lite-{#AppVersion}-x64
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
SetupIconFile=..\..\assets\brand\patentforge.ico
WizardStyle=modern
; A running copy must be closed before files are replaced.
CloseApplications=yes
RestartApplications=no

[Files]
Source: "..\..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-PROMPTS"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LEGAL_NOTICE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs
Source: "..\..\prompts\*"; DestDir: "{app}\prompts"; Flags: ignoreversion recursesubdirs
Source: "..\..\assets\brand\patentforge.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\brand\app-256.png"; DestDir: "{app}\assets"; Flags: ignoreversion
Source: "..\..\assets\fonts\Barlow-Regular.ttf"; DestDir: "{app}\fonts"; Flags: ignoreversion
Source: "..\..\assets\fonts\Barlow-Bold.ttf"; DestDir: "{app}\fonts"; Flags: ignoreversion
Source: "..\..\assets\fonts\OFL.txt"; DestDir: "{app}\fonts"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Run]
Filename: "{app}\{#AppExe}"; Description: "Launch {#AppName}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Only generated files. The user's profile at %LOCALAPPDATA%\patent-forge-lite is
; deliberately preserved so a reinstall or upgrade keeps history. A user-initiated
; "remove my data" action is a separate product decision.
Type: filesandordirs; Name: "{group}"
