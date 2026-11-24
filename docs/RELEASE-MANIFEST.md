# Release manifest

Built from one revision. Every hash below was computed on this machine; re-run the
commands in *Reproducing* to check them.

| Field | Value |
| --- | --- |
| Revision | `1066f7d` |
| Branch | `main` |
| Candidate version | `0.2.0` |
| Licence | MIT (application); prompts CC BY-SA 4.0 |

## Artifacts

| Platform | File | Version | SHA-256 | Bytes | Signed |
| --- | --- | --- | --- | --- | --- |
| Windows x64 | `patent-forge-lite-0.2.0-x64.exe` | 0.2.0 candidate | `dacdfe8c7e75a2c80a4262aa4624387c0d823e101a35c66fde8b619b64a34e74` | 12,024,692 | **no** |
| Windows x64 | `patent-forge-lite-0.1.0-x64.exe` | 0.1.0 released artifact, unchanged | `624f731cea661176ae9989bbba6234ee633af3d8b49314e0c151307b340c7f37` | 7,837,495 | **no** (the signed copy is `dist/signed/`) |

macOS and Linux have **no artifact from this revision**. Their installed acceptance is
blocked on hardware; `docs/PLATFORM-RUNBOOK.md` is the deliverable for both.

## One installer, two profiles

The Windows installer carries both profiles. The operator chooses Full or Lean at first
run; Lean refuses a local runtime and Full offers both a local runtime and the paid Cloud
provider. There is no separate Lean package.

## Version skew, stated plainly

The candidate installer is named **0.2.0** by overriding `AppVersion` at compile time
(`ISCC /DAppVersion=0.2.0`), while `crates/*/Cargo.toml` still declares `0.1.0`. The v0.1.0
release and tag are kept as an accepted decision, and re-issuing an artifact called 0.1.0
from a later revision would silently replace a published file. Choosing the next release
version, and bumping the crates to match, is Scott's decision; until it is made this
manifest is the record of the skew rather than a claim that it does not exist.

## Signature status

Nothing in this candidate is signed or notarised. The signed v0.1.0 copy remains at
`dist/signed/patent-forge-lite-0.1.0-x64.exe` and is unaffected. Signing a new revision
requires approval.

## Reproducing

```sh
cargo build -p pfl-desktop --features native --release
cp target/native/release/pfl-desktop.exe target/release/pfl-desktop.exe
"%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" /DAppVersion=0.2.0 packaging\windows\patent-forge-lite.iss
Get-FileHash dist\patent-forge-lite-0.2.0-x64.exe -Algorithm SHA256
```

The build is not bit-reproducible across machines (Rust does not promise that), so the
hash identifies *this* artifact, not a recipe that will always produce it.
