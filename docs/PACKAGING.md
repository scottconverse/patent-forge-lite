# Packaging

Packaging source exists for all three platforms. The **Windows installer is built and
install-accepted**; the 0.2.0 candidate is unsigned, while the published v0.1.0 release was
signed. macOS and Linux were built and published for v0.1.0 by the release workflow, but **no
installed or interactive acceptance has been performed on either**, and this machine cannot
build them.

| Platform | Source | Tooling | State |
| --- | --- | --- | --- |
| Windows x64 | `packaging/windows/patent-forge-lite.iss` | Inno Setup 6 (`%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe`) | **Built from `8c51ca1`**: `dist\patent-forge-lite-0.2.0-x64.exe`, SHA-256 in `docs/RELEASE-MANIFEST.md`, install/upgrade/rollback/uninstall verified. Unsigned. |
| macOS arm64 | `packaging/macos/Info.plist` + `bundle.sh` | A real macOS host | Not built; no runner available |
| Linux x86_64 | `packaging/linux/pfl-desktop.desktop` | A real Linux host | Not built; no runner available |

## Windows build

```powershell
cargo build -p pfl-desktop --features native --release --locked
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" /DAppVersion=0.2.0 packaging\windows\patent-forge-lite.iss
Get-FileHash dist\patent-forge-lite-0.2.0-x64.exe -Algorithm SHA256
```

`/DAppVersion` is not optional in a recipe: without it the installer is named and versioned
0.1.0, so following this page used to overwrite the published reference artifact that the
upgrade proof uses as its old installer. Always state the version, and record the hash in
`docs/RELEASE-MANIFEST.md`.

The installer is per-user (`PrivilegesRequired=lowest`, installs to
`%LOCALAPPDATA%\Programs\PatentForge Lite`), matching the application's own
per-user data model. It never touches `%LOCALAPPDATA%\patent-forge-lite`, so
uninstalling and reinstalling keeps project history. `dist/` is a build output and
is git-ignored; record the SHA-256 of any artifact you publish.

## Decisions deliberately not made here

1. **Signing a new revision.** The release workflow signs the Windows installer with Azure
   Trusted Signing when the credentials are present and withholds an unsigned installer rather
   than publishing one; that is what signed v0.1.0. A local build from this page is unsigned,
   and signing any new revision needs Scott's approval.
2. **macOS notarization.** The legacy DMG was unsigned; Lite makes no trust claim.
3. **macOS support floor.** `Info.plist` declares 13.0 — Lite's own floor, to be
   verified on a real runner rather than inherited from the legacy tray.
4. **Upgrades.** Exercised and recorded in `docs/evidence/upgrade/`: install, upgrade to the
   candidate, roll back, and the workspace inventory unchanged at each step. The proof refuses
   to run on a machine that already has Patent Forge Lite installed, installs without shortcuts
   or tasks, and removes its own install on failure as well as success.
5. **Distribution of prompts and fonts.** The installer ships `prompts/` with its
   CC-BY-SA notices and `fonts/` with the SIL OFL text, because dropping either
   would break the licence obligations.

## What packaging does not cover

- Automatic updates.
- A tray-resident background lifecycle (not implemented; closing the window exits, and a second instance is refused with exit code 3).
- OS integration beyond shortcuts and the uninstall entry.
- Any migration of existing PatentForge installations.