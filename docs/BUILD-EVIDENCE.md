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

After the storage, workflow and desktop lanes were actually merged, and the CLI
harness was written, the following was measured on this machine with the pinned
`1.98.1` toolchain, `--locked --offline`, and `jobs = 2`:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed (all crates). |
| `cargo test --workspace --locked --offline` | Passed: domain 4, storage 9 migration + 15 persistence, workflow 3 unit + 9 workflow + 1 lifecycle + 1 imported-report, CLI 4 end-to-end. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed. |
| `cargo clippy -p pfl-desktop --features native --all-targets ...` | Passed. |
| `cargo check -p pfl-desktop --features native --locked --offline` | Passed. First real compile of the native app source, not a dependency stub. It found one real GPUI API mismatch (`FocusHandle::focus` now takes `cx`), which is fixed. |
| Desktop portable + actor suite | 23 passed: 8 editor, 8 state, 7 session. This includes the headless actor tests that were previously gated behind `--features native`, and a new app-wide active-job count the quit guard uses. |
| CLI hard-kill recovery | Passed in a genuinely fresh process: killed before the first checkpoint and after the third, then resumed with no completed stage repeated and exactly `7 - stage` provider calls in the resuming process. |
| CLI shared-profile lock | Passed: while the paused child still owned the profile, a second CLI on the same workspace was refused with a lock conflict; the OS lock released on hard kill and recovery then succeeded. |
| Imported edited report | Passed: the import journey exports the original `Edited café report` before any provider exists, then creates version 3 without regenerating it, and the stored history still has 3 versions. |
| Prompt identity | The seven feasibility prompt files are byte-identical (SHA-256) to the frozen legacy `services/feasibility/src/prompts` sources. |

Still not established by this evidence:

- No native window was launched; keyboard, IME, GPU rendering, dialogs and
  installed behavior remain unverified interactive acceptance.
- No live Ollama or Cloud inference ran. Every result above is recorded replay;
  inference orchestration is proven, model quality is not.
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
