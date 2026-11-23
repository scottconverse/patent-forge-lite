# Packaging

Packaging source exists for all three platforms. The **Windows installer is now
built and install-accepted**; it is unsigned. Nothing has been built, signed or
installed for macOS or Linux.

| Platform | Source | Tooling | State |
| --- | --- | --- | --- |
| Windows x64 | `packaging/windows/patent-forge-lite.iss` | Inno Setup 6 (`%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`) | **Built from `7453174`**: `dist\patent-forge-lite-0.2.0-x64.exe`, SHA-256 in `docs/RELEASE-MANIFEST.md`, install/upgrade/rollback/uninstall verified. Unsigned. |
| macOS arm64 | `packaging/macos/Info.plist` + `bundle.sh` | A real macOS host | Not built; no runner available |
| Linux x86_64 | `packaging/linux/pfl-desktop.desktop` | A real Linux host | Not built; no runner available |

## Windows build

```powershell
cargo build -p pfl-desktop --features native --release --locked   # ~7.5 min
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" packaging\windows\patent-forge-lite.iss
```

The installer is per-user (`PrivilegesRequired=lowest`, installs to
`%LOCALAPPDATA%\Programs\PatentForge Lite`), matching the application's own
per-user data model. It never touches `%LOCALAPPDATA%\patent-forge-lite`, so
uninstalling and reinstalling keeps project history. `dist/` is a build output and
is git-ignored; record the SHA-256 of any artifact you publish.

## Decisions deliberately not made here

1. **Signing.** Scott has both a GitHub-hosted SignPath path and Azure Trusted
   Signing. The installer contains no `SignTool` directive, so neither policy is
   implied. Until one is chosen the package is unsigned and SmartScreen will warn.
2. **macOS notarization.** The legacy DMG was unsigned; Lite makes no trust claim.
3. **macOS support floor.** `Info.plist` declares 13.0 — Lite's own floor, to be
   verified on a real runner rather than inherited from the legacy tray.
4. **Upgrades.** No version-to-version upgrade path has been exercised; only a
   clean install and a clean uninstall.
5. **Distribution of prompts and fonts.** The installer ships `prompts/` with its
   CC-BY-SA notices and `fonts/` with the SIL OFL text, because dropping either
   would break the licence obligations.

## What packaging does not cover

- Automatic updates.
- A tray-resident background lifecycle (not implemented; closing the window exits, and a second instance is refused with exit code 3).
- OS integration beyond shortcuts and the uninstall entry.
- Any migration of existing PatentForge installations.