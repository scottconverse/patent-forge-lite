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