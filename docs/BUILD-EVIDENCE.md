# Build and acceptance evidence

## Foundation

Base bootstrap: `bb48ae813242bd3ed4403d5da9aeecaf901d80a4`.
Foundation: `164670a5921032d145bc3ff2e05fe6aaceee8cda`.

Selected versions:

- Rust `1.98.1` (`48a229cea`, 2026-09-01), matching the pinned upstream toolchain.
- Official GPUI and gpui_platform source revision
  `2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4`.
- Native targets planned from the start: Windows `x86_64-pc-windows-msvc`,
  macOS `aarch64-apple-darwin`, Linux `x86_64-unknown-linux-gnu`.
- Cargo.lock records the resolved dependency versions; `jobs = 2` bounds native
  build concurrency. Portable tests may use a separately owned jobs=1 slot.

On 2026-11-21, Windows x86_64 with MSVC C++ Build Tools and Windows SDK:

| Check | Result |
| --- | --- |
| `cargo fmt --all` | Passed at foundation. |
| `cargo test -p pfl-domain` | Foundation: 2 passed, compilation 16.31 seconds. Context delta `beba232`: 4 passed, 3.29 seconds. |
| Portable desktop input/state | 14 passed (8 editor, 6 state) before the nine-field context delta; compilation 1m46s. Cached `--locked --offline` rerun confirmed exit 0. |
| Staged whitespace check | Passed. |
| Original root source/prompt license and legal notice retention | SHA-256 byte identity verified against the pinned original. |
| Initial native GPUI dependency/stub check | Completed in 5m06s; cached `--locked` confirmation exited 0 in 2.88 seconds. Prerequisite only, not real-app acceptance. |
| Real integrated native desktop build | Not run yet. |
| GPUI native interaction | Not run, interactive session not authorized. |
| macOS/Linux native builds | Blocked: no native runner activated; no cloud job dispatched. |
| Installed packages, signing, release | Not run and not claimed. |
| Live local or Cloud inference | Not run and not authorized by these tests. |

The initial dependency build is integration-owned, started at
2026-11-21T13:21:34.741Z, and limited to two compiler jobs. Its managed job tree
reached its terminal result. The initial managed notification did not supply an
exit code; a cached foreground check supplied the explicit exit-0 confirmation.
A successful dependency/stub check
does not validate the later real application binary, window behavior, renderer
availability, model quality, packages or installed use.

## Integrated evidence, 2026-11-21 (Windows x86_64 only)

After all four workflows were implemented on top of the storage, workflow and
desktop lanes, and the CLI harness and compatibility API were written, the
following was measured on this machine with the pinned `1.98.1` toolchain,
`--locked --offline`, and `jobs = 2`:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed (all crates). |
| `cargo test --workspace --locked --offline` | Passed: **96 tests** — domain 8, storage 24, workflow 8 unit + 9 + 1 + 1 + 7 + 5, CLI 6, desktop 24, API 3. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed. |
| `cargo clippy -p pfl-desktop --features native --all-targets ...` | Passed. |
| `cargo check -p pfl-desktop --features native --locked --offline` | Passed. Real compile of the native app source. |
| `cargo build -p pfl-desktop --features native --locked --offline` | Passed and linked `target/debug/pfl-desktop.exe` (32.76 MiB). The binary was not launched. |
| Workflows end to end | `pfl-cli pipeline` completes all four workflows in dependency order: 19 stages, 19 recorded provider calls, then reopens the profile with zero further generation. |
| Word export | A `.docx` container is opened back up in tests and its `word/document.xml` contains the saved report text. |
| Provider wire formats | Cloud SSE and Ollama NDJSON are exercised against local mock servers; unauthorized cloud runs make no request at all. |
| Research seam | Supplied references are snapshotted into the run input and reach the prior-art prompt; unauthorized runs query nothing. |
| Setup profiles | Lean refuses a local runtime; Full accepts it. |
| HTTP compatibility | Bearer, both legacy 401 messages, tokenless mode, long-lived query token, and single-use 5-minute download tokens verified. `pfl-cli serve` was exercised as a real process over a seeded profile. |
| CLI hard-kill recovery | Passed in a genuinely fresh process at stage 1 and stage 4, with no repeated checkpoints. |
| CLI shared-profile lock | Passed: a second owner is refused while the first holds the profile; the OS lock releases on hard kill. |
| Prompt identity | All seven feasibility prompts, plus the claims, compliance and application sets, are byte-identical (SHA-256) to the frozen legacy sources. |
| **Live on-machine inference** | **Passed**, 2026-11-21. `pfl-cli live` against the already-running local Ollama at `http://127.0.0.1:11434` with `phi4-mini:latest`, synthetic fixture input only. All four workflows completed in dependency order: **19 stages, 19 real provider calls, 715.5 s**. Saved reports: feasibility 12 212 B, claims 34 678 B, compliance 66 904 B, application 56 040 B. The profile reopened with four versions and zero further generation. No model was downloaded and no independently owned process was started or stopped. An earlier feasibility-only live run (six stages, 189.5 s) is also recorded. |

### What the live runs showed

The model produced coherent, on-topic material — real numbered claims with
broad/medium/narrow independents and dependents, and a full application with
abstract, background, summary, drawings listing, detailed description and an IDS
table. It also **invented prior-art reference numbers** (for example "12345678 /
Temperature Sensor in Coffee Cup") because no research source was configured. That
is precisely the failure mode the embedded disclaimers warn about, and it is why
the output is labelled research material rather than verified prior art. Real
research remains a separate integration.

Still not established by this evidence:

- No native window was launched; keyboard, IME, GPU rendering, dialogs and
  installed behavior remain unverified interactive acceptance. A linked binary is
  a build result, not usability evidence.
- No live **Cloud** inference ran; the adapter is verified against a mock only and
  remains fail-closed without explicit paid authorization.
- macOS and Linux native builds remain unattempted on this machine.
- Packaging, signing, release and the manual CI workflow remain unrun.

## Reproduction

```powershell
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build -p pfl-desktop --features native --locked
```

Do not launch the native binary on an occupied desktop without authorization.
The manual-only workflow defines native build jobs for all three targets; its
existence is not evidence that any job ran or that a hosted runner is available.
Any cloud run retains its explicit runner/cost authorization gate.

## Windows installer, 2026-11-21

First package ever built for this product. **Unsigned** — no signing policy has
been chosen yet, so SmartScreen will warn on first run.

| Fact | Value |
| --- | --- |
| Build command | `cargo build -p pfl-desktop --features native --release --locked` (7m27s) then `iscc packaging\windows\patent-forge-lite.iss` (6.2s) |
| Inno Setup | 6, at `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe` |
| Artifact | `dist\patent-forge-lite-0.1.0-x64.exe`, 7.15 MB |
| SHA-256 | `10A165F366EAF0FBE18C3C26A938A5C299F9A6960F822F5C6A9E5286EF17DC77` |
| Install scope | Per-user, no elevation: `%LOCALAPPDATA%\Programs\PatentForge Lite` |
| Installed payload | `pfl-desktop.exe` (19.6 MB), docs, all prompt files, Barlow fonts with OFL, brand icon |

Verified on this machine:

1. Silent install (`/VERYSILENT /DIR=...`) exited 0 and wrote the uninstall key
   under `HKCU`.
2. The **installed** binary launched, exposed its accessibility tree (15 nodes) and
   **passed all 26 UI acceptance checks** when the suite was pointed at the
   installed path (`scripts/ui-acceptance.ps1 -Exe ...`).
3. Start-menu shortcut created.
4. Silent uninstall exited 0, removed the install directory and shortcut, and
   **preserved** `%LOCALAPPDATA%\patent-forge-lite` (the user's project history).

Not done: code signing, an upgrade path across versions, macOS/Linux packages.

## GitHub Actions dispatch, 2026-11-21

The manual native workflow was dispatched once and **did not run**: all three jobs
were rejected before starting with

> The job was not started because recent account payments have failed or your
> spending limit needs to be increased.

That is a GitHub billing condition on the account, not a workflow or code failure.
`workflow_dispatch` triggered (run 44949798709), so the definition itself is
reachable; no runner ever executed a compile. macOS and Linux therefore remain
**never compiled**, and the platform row stays open until billing is resolved.
## Cross-platform CI, 2026-11-21

The repository was made public (unblocking GitHub's billing restriction on private
Actions minutes) and the manual native workflow was dispatched:
run [`44950601970`](https://github.com/scottconverse/patent-forge-lite/actions/runs/44950601970).

| Runner | Target | Result | Time |
| --- | --- | --- | --- |
| `windows-latest` | `x86_64-pc-windows-msvc` | passed | 8m51s |
| `macos-15` | `aarch64-apple-darwin` | passed | 6m02s |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | passed | 4m59s |

Each job ran `cargo fmt --all -- --check`, `cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo build -p pfl-desktop --features native --locked` for its own target. All
three passed on the first attempt, so the code compiles and its tests pass on
macOS arm64 and Linux x86_64 — previously never compiled at all.

What this still does not establish: no installed package, no launched window and no
interactive behaviour on macOS or Linux. Those remain separate acceptance steps.

### History scrub and a residual exposure

The public repository history was rewritten before publishing to remove the
screenshots committed by mistake on 2026-11-21 (including a full-desktop capture
showing unrelated windows and personal folder names). `filter-branch` plus a full
`gc --prune=now` removed them from every ref; a fresh clone of the public remote
contains only `assets/brand/app-256.png`.

Residual and unresolved: GitHub still serves the pre-rewrite commit
`423c2ead695099ba96807a5ec697a960cdde4cd4` by its ID, and that tree still contains
`session-screen.png` and the early `shots/*.png`. Unreachable objects are not
immediately garbage-collected and no API is exposed for forcing it. The SHA was
never published or linked, so the practical exposure is low, but it is not zero.
Options: ask GitHub Support to purge unreachable objects, or delete and recreate
the repository (same name and URL, clean object store, empty Actions history).
## Release v0.1.0, 2026-11-21

Tags and publishes through `.github/workflows/release.yml` (run 44953930049).

| Asset | Size | State |
| --- | --- | --- |
| `patent-forge-lite-v0.1.0-linux-x64.tar.gz` | 16.44 MB | Published, unsigned; sha256 sidecar verified against the download |
| `patent-forge-lite-v0.1.0-macos-arm64.zip` | 7.74 MB | Published, unsigned; contains a real Mach-O arm64 `pfl-desktop` (magic `cf fa ed fe`) |
| Windows installer | — | **Not published**: the repository has no Azure secrets yet, and the pipeline refuses to publish an unsigned Windows installer |

Both artifacts were downloaded again from the public release and inspected: the Linux tarball
carries the binary, docs, all prompt files, Barlow fonts, the `.desktop` entry and notices; the
macOS zip is a well-formed `.app` bundle. CI: fmt, tests, clippy and the native build pass on all
three runners; the macOS and Linux jobs each produced their package in CI.

Landing page: https://scottconverse.github.io/patent-forge-lite/ (GitHub Pages, `main:/docs`).

Remaining gate: signing the Windows installer requires `AZURE_CLIENT_ID`, `AZURE_TENANT_ID` and
`AZURE_CLIENT_SECRET` in this repository, mirroring the CivicCast pipeline (account
`scottconverse-signing`, profile `ScottConversePublic`, endpoint `wcus.codesigning.azure.net`).
With those present the Windows job builds, signs, verifies and the publish step attaches the
installer to the existing v0.1.0 release.
## Windows signing, 2026-11-21 (Azure Trusted Signing)

Signed through `scottconverse/civiccast` -> `.github/workflows/sign-native-installer.yml`,
per the one full signing procedure. Two runs, one per file, in the required order (app binary
first, then a rebuilt installer that contains it).

| File | Unsigned SHA-256 | Signed SHA-256 | Run |
| --- | --- | --- | --- |
| `pfl-desktop.exe` (application) | `9738f5d7…ee423` | `7abf0c77…b8a71` | [44965745433](https://github.com/scottconverse/civiccast/actions/runs/44965745433) |
| `patent-forge-lite-0.1.0-x64.exe` (installer) | `3cdcdcfa…991f2` | `84da4443…a194edbb` | [44966583618](https://github.com/scottconverse/civiccast/actions/runs/44966583618) |

Both: `Status: Valid`, signer `CN=Scott Converse, O=Scott Converse, L=Longmont, S=co, C=US`,
issuer `Microsoft ID Verified CS EOC CA 03`, RFC3161 timestamp from `Microsoft Public RSA Time
Stamping Authority`, and each file's SHA-256 equals `signed_sha256` in its receipt.

The installer was rebuilt only after `target\release\pfl-desktop.exe` was replaced with the signed
binary, so it embeds the signed executable. Proven by installing it: the installed
`pfl-desktop.exe` hashes to `7abf0c77…b8a71` and its signature status is `Valid`. Uninstall
removed the application and preserved the profile.

Draft intake releases created in civiccast, left as drafts and never published:

- `patentforgelite-intake-9738f5d7-20261121-171547` (app)
- `patentforgelite-intake-3cdcdcfa-20261121-172604` (installer)

Published asset: `patent-forge-lite-0.1.0-x64.exe` (7.86 MB) plus `.sha256` on the v0.1.0
release, re-downloaded and re-verified after upload.

One process note, recorded because it happened: the first run used an earlier helper script that
failed to locate the run it had dispatched and stopped. It had already dispatched a successful
run; that run was recovered from the run list and verified (never re-dispatched). The full
procedure's script replaced it for the installer and printed `SIGNED OK`.