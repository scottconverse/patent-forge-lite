# Handoff

State of the work for whoever picks this up. Everything here is checkable in the
repository; where a thing is blocked, the blocker is named and nothing claims otherwise.

## Repository

`C:\Users\scott\Documents\Codex\PatentForgeLite` — public `scottconverse/patent-forge-lite`,
branch `main`. The session working directory may differ: **run every cargo command with the
project as its working directory**.

Crates: `pfl-domain` (types), `pfl-storage` (SQLite, migration), `pfl-workflow` (workflows,
providers, research, package, claims, diff, cost), `pfl-api` (read HTTP surface),
`pfl-desktop` (GPUI window and the session actor), `pfl-cli`.

## Gates

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
cargo clippy -p pfl-desktop --features native --offline -- -D warnings
pwsh -File scripts/acceptance-check.ps1
```

Read each command's **own** exit code. Piping cargo into `Select-String` reports the
pipeline, not cargo, and has produced false greens in this work.

## Traps that have cost rounds

- **`CARGO_TARGET_DIR` and tests.** Set the native build to `target\native` and never run
  `cargo test` with that variable still set: a test build writes a non-native stub over the
  binary and every UI proof then fails as if the app were broken. The proofs refuse a stub,
  but the message is easy to misread.
- **Inherited `GIT_CONFIG_COUNT`.** A polluted git environment makes every git command fail
  with `missing config key GIT_CONFIG_KEY_0`. Clear `GIT_*` variables in the shell first.
- **A seeded profile is not a first run.** `scripts/seed-profile.ps1` marks settings
  `profile_chosen`; without that the app correctly shows the profile prompt and every
  installed proof stalls there.
- **Exact-name lookups return the label text**, which exposes no action. Resolve controls
  before text, and prefer `SelectionItem`/`Invoke` over clicking.
- **The pointer** is only needed for controls that expose no action. Navigation no longer
  does: the rail is a tab list.
- **Session tests**: use a plain `#[test]`, not `#[tokio::test]`, because the session owns a
  private runtime and blocking a worker starves it. To observe a run, `SelectRun(id)` and
  read the snapshot; do not wait for `Poll` timing.
- **Cloud stand-in URL**: `PFL_CLOUD_ENDPOINT` is a base URL. The provider appends
  `/v1/messages`; passing the full path produces a run that fails having contacted nothing.

## What is proven, and where

`docs/ACCEPTANCE.md` is the matrix. Read its summary table rather than a count repeated
here: a restated number is exactly how this document fell behind before, and the check
now refuses one. `scripts/acceptance-check.ps1` runs in CI and compares the summary with
the rows, the cited revision and hashes with the release manifest, and scans the
reader-facing documents for stale claims.
evidence paths, stale claims and single-stage wording cannot drift silently.

`docs/BUILD-EVIDENCE.md` is the chronological trail, including the failures and every claim
that had to be corrected.

Per-area evidence lives in `docs/evidence/<area>/<timestamp>/` with its JSON, and for
providers the request log as well.

## Blocked, with owners

| Blocker | Needs |
| --- | --- |
| A5's installed decline | the window proof cannot start a run: pressing Start through that script never reaches the handler, while `installed-local-proof` starts runs at the same revision 5/5. Six attempts across four rounds failed. A human watching the window would settle it in minutes. The behaviour itself is proven headlessly on three platforms: declined → 0 requests, approved → 6. |
| C live PatentsView query | a key, supplied per session. |
| E2 real database import | authorization and a copy of a real database. |
| F3 macOS/Linux installed | hardware; `docs/PLATFORM-RUNBOOK.md` is the deliverable. |

## Decisions that are Scott's

Next release version (installer says 0.2.0, crates say 0.1.0), signing, publication, the
PatentsView key, migration authorization, macOS/Linux hardware, and whether to keep or prune
the `impl/*` branches.

## Running the installed proofs

Each runs a real window and takes the mouse and keyboard; announce before running one.

```
cargo build -p pfl-desktop --features native        # into target\native
pwsh -File scripts/ui-acceptance.ps1 -Exe target\native\debug\pfl-desktop.exe
pwsh -File scripts/installed-local-proof.ps1 -Exe ...   # Local runs, truncation, resume
pwsh -File scripts/cloud-consent-proof.ps1 -Exe ...     # configure, cost line, approval
pwsh -File scripts/editing-proof.ps1 -Exe ...           # edit, revisions, diff, export, regenerate
pwsh -File scripts/models-proof.ps1 -Exe ...            # model picker and readiness
pwsh -File scripts/first-run-proof.ps1 -Exe ...         # profile prompt, Lean refuses Local
pwsh -File scripts/lifecycle-proof.ps1 -Exe ...         # single instance, quit
pwsh -File scripts/research-proof.ps1 -Exe ...          # opt-in research against a mock
pwsh -File scripts/upgrade-proof.ps1 -OldInstaller ... -NewInstaller ...
```

`scripts/full-evidence.ps1` runs all of them; it is long and takes the machine, so prefer
batches when the operator is present.

## Sensible next steps

1. Watch the window while pressing Start after a Cloud configure, and find why that script
   never reaches the handler. If it fires, A5 closes.
2. Convert the remaining pointer paths in proofs to `SelectionItem`/`Invoke`, so GUI runs stop
   moving the cursor.
3. When a key or authorization arrives, run the live PatentsView query and the real import,
   and record both artifacts.