# Packaging

Packaging source exists for all three platforms. **No installer, bundle or package
has been built, signed or installed.** Nothing in this directory is evidence that a
package works.

| Platform | Source | Tooling needed | Built? |
| --- | --- | --- | --- |
| Windows x64 | `packaging/windows/patent-forge-lite.iss` | Inno Setup 6 (`iscc`) | No — Inno Setup is not installed here |
| macOS arm64 | `packaging/macos/Info.plist` + `bundle.sh` | A real macOS host | No — no macOS runner |
| Linux x86_64 | `packaging/linux/pfl-desktop.desktop` | A real Linux host, optional AppImage tooling | No — no Linux runner |

## Build sequence

```powershell
cargo build -p pfl-desktop --features native --release --locked
iscc packaging\windows\patent-forge-lite.iss
```

```bash
cargo build -p pfl-desktop --features native --release --locked
packaging/macos/bundle.sh
```

## Decisions that are deliberately not made here

1. **Signing.** Scott has both a GitHub-hosted SignPath path and Azure Trusted
   Signing. The installer definition intentionally contains no signing directive so
   neither policy is implied. Choose one before shipping.
2. **macOS notarization.** The legacy DMG was unsigned. Lite makes no trust claim
   until a decision is recorded and a real signed build is verified.
3. **macOS support floor.** `Info.plist` currently declares 13.0. This is Lite's own
   floor (no tray-resident lifecycle) and must be verified on a real runner rather
   than inherited from the legacy tray's conflicting 11/12/15 values.
4. **Per-user data on uninstall.** The installer preserves
   `%LOCALAPPDATA%\patent-forge-lite` so upgrades keep history. A "remove my data"
   action is a separate product decision.
5. **Distribution of prompts.** Prompt files are installed alongside the binary so
   their CC-BY-SA notices and attribution travel with the product. Any packaging
   that omits `prompts/` and the notice files would break the license obligations.

## What packaging does not cover

- Automatic updates.
- A tray-resident background lifecycle (not implemented).
- OS integration beyond a shortcut and a `.desktop` entry.
- Any migration of existing PatentForge installations.
