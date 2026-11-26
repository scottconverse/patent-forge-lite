# Release manifest

Built from one revision. Every hash below was computed on this machine; re-run the
commands in *Reproducing* to check them.

| Field | Value |
| --- | --- |
| Revision | `da6bcc5` |
| Branch | `main` |
| Candidate version | `0.2.1` |
| Licence | MIT (application); prompts CC BY-SA 4.0 |

## Artifacts

| Platform | File | Version | SHA-256 | Bytes | Signed |
| --- | --- | --- | --- | --- | --- |
| Windows x64 | `patent-forge-lite-0.2.1-x64.exe` | 0.2.1 release candidate | `1027aa3da9c17746d43dad717195ac2a515fce16e37ca749c61cfe7512c30a23` | 12290547 | **no** |
| Windows x64 | `patent-forge-lite-0.1.0-x64.exe` | 0.1.0 released artifact, unchanged | `624f731cea661176ae9989bbba6234ee633af3d8b49314e0c151307b340c7f37` | 7,837,495 | **no** (the signed copy is `dist/signed/`) |
| Linux x86_64 | `patent-forge-lite-v0.2.1-linux-x64.tar.gz` | 0.2.1 release candidate, built in CI from `3535022` | `7e9046edaa7a112a6915ecdd19423ff7d2817cefe54a9ef3aba66ef3fdcd3a07` | not recorded | **yes** (CI only, no signing key involved) |

macOS and Linux now have artifacts, both built away from this Windows machine: Linux in CI
(tarball above, launched under Xvfb), macOS on the Mac mini (`docs/evidence/macos/20261125T023619Z/`,
unsigned because there is no Apple certificate). What neither has is a human using the window.

## One installer, two profiles

The Windows installer carries both profiles. The operator chooses Full or Lean at first
run; Lean refuses a local runtime and Full offers both a local runtime and the paid Cloud
provider. There is no separate Lean package.

## Version skew, stated plainly

The installer is named **0.2.1** from `AppVersion`
(`ISCC /DAppVersion=0.2.1`), and `crates/*/Cargo.toml` declares `0.2.1` too, so the artifact name and the crate version agree. The v0.1.0
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
"%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" /DAppVersion=0.2.1 packaging\windows\patent-forge-lite.iss
Get-FileHash dist\patent-forge-lite-0.2.1-x64.exe -Algorithm SHA256
```

The build is not bit-reproducible across machines (Rust does not promise that), so the
hash identifies *this* artifact, not a recipe that will always produce it.