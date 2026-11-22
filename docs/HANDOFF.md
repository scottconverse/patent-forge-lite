# Handoff

State of the work for a fresh session. Everything here is verifiable from the
repository; where it says blocked, the blocker is named.

## Repository

`C:\Users\scott\Documents\Codex\PatentForgeLite` — public `scottconverse/patent-forge-lite`, branch `main`.
The session working directory may differ: **run every cargo command with the
project as its working directory**, or it fails with "package ID did not match".

Crates: `pfl-domain` (types), `pfl-storage` (SQLite), `pfl-workflow` (workflows,
providers, research, package, claims, diff, cost), `pfl-api`, `pfl-desktop`
(GPUI), `pfl-cli`.

## Gates

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo test --workspace --offline
```

Read each command's own exit code. Piping into `Select-String` reports the
pipeline's status, not cargo's — this has produced two false "green" claims in
this work.

Build the native UI into a separate target directory so plain tests cannot
overwrite it with a stub:

```
$env:CARGO_TARGET_DIR = "...\PatentForgeLite\target\session"
cargo build -p pfl-desktop --features native --offline
```

## Installed acceptance harnesses

| Script | Purpose | State |
| --- | --- | --- |
| `scripts/ui-acceptance.ps1` | 33 checks on an isolated profile, PID-bound | working |
| `scripts/installed-local-proof.ps1` | real-declared Local run, Truncated, resume (6→3→4 requests) | working |
| `scripts/mock-ollama.ps1` | loopback Ollama stand-in | working |
| `scripts/cloud-consent-proof.ps1` | Cloud configure proven; Start cannot be activated | partial |
| `scripts/mock-cloud.ps1` | loopback Anthropic SSE stand-in | working |

Resolve UI elements **by name first, then substring**; an inner label has no
`InvokePattern`, so substring-only resolution silently does nothing.

## CI

`gh workflow run native-manual.yml --repo scottconverse/patent-forge-lite`, then
wait for the conclusion. A run that stays `in_progress` for more than a few
minutes is wedged: cancel it (`gh run cancel`) and dispatch again.

## Where the work stands

`docs/ACCEPTANCE.md` holds the directive-wide matrix: every A–G item with its
status and evidence class. `docs/BUILD-EVIDENCE.md` holds the chronological
trail, including the failures and the corrections of earlier false claims.

Summary: Phase A complete except A5; Phase B engine complete except B5 execution
(design-blocked); Phase C adapter, opt-in and desktop controls done, live query
blocked on a PatentsView key; Phases D–G largely open.

## Blockers, with their owners

| Blocker | Needs |
| --- | --- |
| A5 installed decline proof | gpui: `role(Button)` div exposes no `InvokePattern`, so automation cannot activate Start. A framework fix, or a manual click-path check. |
| B5 section regeneration | a decision: sparse checkpoints with a carried marker (touches durability) or suffix regeneration (safe, narrower). See `docs/REGENERATION.md`. |
| C live query | a PatentsView API key, supplied per session and never persisted. |
| E2 real import | authorization and a copy of a real PatentForge database. |
| F3 macOS/Linux installed acceptance | hardware; a runbook is required instead. |

## Next three actions

1. **B1 desktop editing**: add a claims/application edit surface that calls the
   existing `WorkflowService::save_revision`, with a headless session test that
   drives a run to terminal through a proper reply-draining helper (not a race
   against `until()`).
2. **B5**, once the semantic decision is made.
3. **D1 model picker** from `list_local_models`, which is already verified
   against the real runtime.

## Rules that cost time to learn

- One edit tool call per change; no multi-edit scripts, no heredocs.
- A new function goes **before** a file's `#[cfg(test)]` module; tests stay last.
- Never claim a gate, a CI run, or an installed behaviour that was not observed.
  Every false claim in this work came from inferring instead of measuring.
