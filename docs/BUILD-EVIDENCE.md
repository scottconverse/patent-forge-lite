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
| `cargo test -p pfl-domain` | Passed: 2 unit tests, 0 failures; compilation 16.31 seconds. |
| Staged whitespace check | Passed. |
| Original root source/prompt license and legal notice retention | SHA-256 byte identity verified against the pinned original. |
| Initial native GPUI dependency/stub check | In progress at this evidence checkpoint, not acceptance. |
| Real integrated native desktop build | Not run yet. |
| GPUI native interaction | Not run, interactive session not authorized. |
| macOS/Linux native builds | Blocked: no native runner activated; no cloud job dispatched. |
| Installed packages, signing, release | Not run and not claimed. |
| Live local or Cloud inference | Not run and not authorized by these tests. |

The initial dependency build is integration-owned, started at
2026-11-21T13:21:34.741Z, and limited to two compiler jobs. Its managed job tree
can be cancelled by the build owner only. A successful dependency/stub check
does not validate the later real application binary, window behavior, renderer
availability, model quality, packages or installed use.

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
