# Provider choice: bounded implementation plan

Status: proposed. Owner: Scott. Author: the implementation agent.
Written 2026-11-25 against `main` at the commit that added `OpenAiCompatibleProvider`.

This plan ports the **decisions** of TownReporter's model system (registry, per-job assignment,
CLI-based subscription sign-in, local discovery, failover receipts) into PatentForgeLite. It does
not port its code: the stacks, the database and the UI toolkit are different.

## 1. Goal

An operator can pick, for each stage of each workflow, which model runs it — from a list the
application discovers itself:

- local servers already running on this machine (LM Studio, Ollama, llama.cpp, vLLM);
- hosted APIs with a key (Anthropic today; any OpenAI-compatible endpoint);
- subscription CLIs already signed in on this machine (`claude`, `codex`, `gemini`);
- or `Automatic`, which probes a ladder and pins whichever rung answers, leaving a receipt.

## 2. Bounded scope

**In scope**

- A pure provider registry: kinds, entries, budgets, per-surface offers, three id lists
  (picker / internal / retired).
- Per-stage model assignment persisted, with ordered fallbacks (rank 0, 1, 2).
- Transports: Claude Code CLI, Codex CLI, Gemini CLI, Anthropic HTTP, OpenAI-compatible HTTP,
  Ollama HTTP.
- Subscription sign-in by spawning the vendor CLI; no credential is read, stored or logged.
- Local discovery over a fixed address list, distinguishing **loaded** from merely downloaded.
- Automatic ladder with per-run receipts (requested · actual · skipped).
- A **Models** page: provider status, model lists, per-stage assignment, sign-in buttons.
- A `pfl-cli models` command that lists providers, models and sign-in state (testable headless).
- Documentation, matrix rows and evidence for every item above.

**Out of scope (deliberately)**

- Signing, publishing, tagging, version decisions.
- Live paid verification (needs Scott's key; separate instruction).
- Custom user-defined API connections (a base URL + key typed by the operator) — TownReporter has
  this; it is a second phase.
- Vision/OCR surfaces and image models.
- Multiple workspaces/newsrooms; the equivalent scoping dimension here is the project.
- Starting or supervising local servers (never load or unload a model — copied rule).
- Per-run override UI (choosing a model for one run without changing the assignment) — phase two.
- Effort controls for transports that do not accept a run-scoped setting.

## 3. Decisions copied from TownReporter (not invented here)

| # | Decision | Why |
|---|---|---|
| D1 | One pure registry module; a new provider is an entry, not a new file | `provider-registry.ts` header: four files had to agree by hand, and one surface was missed |
| D2 | Three id lists incl. **retired** ids | a stored value from an older build must load without error and normalise to Automatic |
| D3 | `Automatic` is an instruction, not an entry | it has no model, budget or transport; special-casing it everywhere is the alternative |
| D4 | Budgets live on the entry, with a generous `local` default | measured: CLI reloads a ~25k preamble per call; a 30B local model answers in minutes |
| D5 | Subscription access = spawn the vendor CLI | `claude auth login --claudeai` needs no TTY; `codex login --device-auth` prints URL + code. The CLI owns its credential file and refresh |
| D6 | Never `codex login` plain | it binds a fixed localhost:1455; two attempts collide |
| D7 | One open sign-in per provider | two device flows print two codes and only one works |
| D8 | `detail` is redacted before storage | a CLI's last stderr line is not a promise |
| D9 | `loaded` ≠ downloaded; never load/unload a model | LM Studio lists everything on disk; the app must not disturb the operator's own use |
| D10 | Receipt records requested · actual · skipped | a reader must always know what wrote the text |
| D11 | Error taxonomy: quota/unavailable/login-lapse/timeout/no-output/unreadable **advance**; refusal/unrecognised **stop** | advancing on a refusal silently hides a content problem |

## 4. Tasks

Each task is one commit. Format: **What**, **How**, **Done when**.

### T1 — Provider registry (pure, no I/O)

- **What** `crates/pfl-workflow/src/registry.rs`: `ProviderKind`, `ProviderEntry`, `PROVIDER_REGISTRY`,
  `PICKER_PROVIDER_IDS`, `INTERNAL_PROVIDER_IDS`, `RETIRED_PROVIDER_IDS`, `KIND_BUDGETS`,
  `providers_for(surface)`, `entry(id)`, `automatic_ladder()`.
- **How** Surfaces are the 19 stages plus a workflow-level default. Entries declare model, label,
  detail, budget, `offered_for`, `offered_off_machine`, ladder rank, optional `requires_loaded_local_model`
  and `picks_loaded_local_model`. No `tokio`, no `std::env` reads except through an injected
  `Env` struct, so it compiles into the GUI.
- **Done when** unit tests prove: every picker id has an entry; every entry's id is in exactly one
  list; the ladder order equals the ranks; budgets are non-zero and per-kind; a grep test fails if
  a provider id string appears outside `registry.rs`; a retired id resolves to `None` and
  normalises to Automatic.

### T2 — Widen the provider identity, without breaking stored runs

- **What** `ProviderKind` gains `ClaudeCode`, `Codex`, `GeminiCli`, `OpenAi`, and keeps `Local`,
  `Cloud` (Anthropic) and `Replay`. `ProviderSnapshot` gains `effort: Option<Effort>` and keeps
  `endpoint`.
- **How** serde aliases so a run written by 0.2.1 (`"cloud"`, `"local"`, `"replay"`) still
  deserialises; a stored unknown kind maps to `Replay` **with a warning recorded on the run**, never
  a hard failure. `authorize` gains one arm per kind: off-machine kinds require synthetic data;
  metered kinds require paid approval; subscription CLIs require explicit authorization but are not
  metered.
- **Done when** a payload captured from 0.2.1 round-trips; the authorize matrix has a test per
  (kind × authorization) pair; an unknown kind opens with a warning and does not start a run.

### T3 — Claude Code transport

- **What** `providers/cli_claude.rs`: spawn `claude`, parse `--output-format json`, stream
  `stream-json` events where used, kill the child tree on cancel.
- **How** argv copied from `ai-claude-code.server.ts:buildClaudeCodeArgs` (system prompt file or
  inline, `--model`, optional effort, `--no-session-persistence`, settings that disable CLAUDE.md,
  hooks, auto-memory and attribution). `stdin` closed; stdout captured; stderr redacted; wall and
  call timeouts from the entry budget.
- **Done when** tests run against a **fixture CLI** (a script emitting captured real output) for:
  success, non-zero exit, timeout, cancellation mid-stream, and an auth-failure shape; a live test
  runs only when `claude` is on PATH and is `#[ignore]`d otherwise; no credential appears in any
  log line (grep test).

### T4 — Codex transport

- **What** `providers/cli_codex.rs`, same trait and guarantees.
- **How** argv and output parsing copied from `ai-codex.server.ts` **after reading it in full**
  (not yet read at plan time — first step of the task).
- **Done when** the same five fixture tests pass; a live opt-in test proves a signed-in `codex`
  answers; cancellation kills the tree on Windows and POSIX.

### T5 — Gemini CLI transport

- **What** `providers/cli_gemini.rs`, same trait.
- **How** copied from `ai-gemini-cli.server.ts`; the entry is `offered_for` every surface except any
  vision surface.
- **Done when** fixture tests pass; if no `gemini` binary is available in CI or here, the matrix row
  is **blocked — harness** with that reason, not pass.

### T6 — Subscription sign-in

- **What** `provider_logins` table, `start_login` / `poll_login` / `cancel_login` / `probe_login`,
  and the argv: `claude auth login --claudeai`, `codex login --device-auth`.
- **How** spawn detached, stdin closed, capture stdout/stderr; parse the authorize URL and (Codex)
  the one-time code with the same shapes TownReporter's captured fixtures show; `detail` redacted
  before insert; statuses `starting | awaiting_user | done | failed | expired | cancelled`; one open
  attempt per provider; expiry 10 min (Claude) / 15 min (Codex).
- **Done when** parser tests use **captured real CLI output** (ANSI included) and assert the
  failure cases from `provider-login.test.ts`: prompt line is not a URL, banner version is not a
  code, punctuation trimmed; a test asserts no token-shaped string is stored; `probe_login` reports
  false when the CLI is absent.

### T7 — Local discovery

- **What** probe `127.0.0.1:1234`, `:11434`, `:8080` (and any configured override); list models with
  `loaded`, kind and embedding classification; "use whatever is loaded".
- **How** one address list in the registry, one prober in the workflow crate, OpenAI catalogue first
  (already implemented) then Ollama's `/api/tags`; cached with a short TTL; **never** loads or
  unloads.
- **Done when** mock-server tests cover LM Studio shape, Ollama shape, a server that answers neither,
  and `loaded` selection; a live test against a running LM Studio/Ollama is opt-in and recorded; the
  picker's "use whatever is loaded" resolves at call time and writes the model into the receipt.

### T8 — Persistence: assignments and settings

- **What** `model_assignments(project_id, surface, rank, provider_id, model)` and
  `provider_settings(base_url, off_switch, budget overrides)`.
- **How** a schema migration in the existing style; a missing row means "Automatic", exactly as
  TownReporter's unassigned job keeps old behaviour; rank 0 is the choice, 1–2 fallbacks.
- **Done when** CRUD tests pass; a database written before this migration opens and reports every
  surface unassigned; deleting a project cascades; the acceptance row cites the migration.

### T9 — Automatic ladder, failover and receipts

- **What** `plan_failover(...)` (pure) plus the executor: try the assigned rank, then the ladder;
  write `RunReceipt { requested, actual, effort, skipped: Vec<{rung, reason}> }`.
- **How** port of `planAutomaticFailover` and the error taxonomy; the receipt is persisted on the
  run snapshot so it survives reopen.
- **Done when** unit tests cover each taxonomy entry (advance vs stop); an integration test runs two
  fake providers where the first fails with a quota error and the second succeeds, asserting the
  receipt names the skip reason; a resumed run does not re-probe from the top.

### T10 — Models page

- **What** A page listing every provider with status (signed in / not installed / no answer), its
  models, a sign-in button when an auth failure is detected, and a per-stage assignment control with
  fallbacks.
- **How** GPUI, following the existing Setup page's idioms; the assignment control reads
  `providers_for(stage)`; unavailable providers are disabled with a reason, never hidden silently.
- **Done when** the UI harness asserts: the page renders one row per registry entry; choosing a
  model persists it and a later run requests it; "Automatic" is selectable and leaves no model
  pinned; a sign-in button appears only for an auth-shaped failure; screenshots recorded.

### T11 — `pfl-cli models`

- **What** `pfl-cli models --list` / `--probe` / `--set <surface> <provider> [--fallback ...]` /
  `--sign-in <claude|codex>` printing JSON.
- **How** the same session actions the GUI uses, headless, so CI can test the whole path without a
  window.
- **Done when** CI runs `models --list` and asserts the registry matches the binary's output; a test
  sets an assignment through the CLI and reads it back via the GUI's own reader.

### T12 — Record and release hygiene

- **What** manual section, `docs/PROVIDERS.md`, matrix rows with evidence, `docs/BUILD-EVIDENCE.md`
  entries, and the installer rebuilt because shipped code changed.
- **How** each row cites a test or an evidence directory; the freshness check will force the
  installer rebuild.
- **Done when** the acceptance checker passes with the new rows; the installer is rebuilt at the
  final code revision and the upgrade proof passes; CI is green on all three platforms.

## 5. Sequence and gates

```
T1 → T2 → { T3, T4, T5 } → T6 → T7 → T8 → T9 → T10 → T11 → T12
```

T3/T4/T5 may go in any order; everything else is strictly ordered because each depends on the
previous identity or table existing.

Every task's commit must pass, in this order:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p pfl-desktop --features native --all-targets -- -D warnings
cargo test --workspace
cargo test -p pfl-desktop --features native --lib
pwsh ./scripts/acceptance-check.ps1
```

and CI must be green on Windows x64, macOS arm64 and Linux x86_64 before the next task starts.

## 6. Risks, and what is done about each

| Risk | Mitigation |
|---|---|
| Vendor CLI argv drifts | fixtures are captured from the real CLI and re-captured when a probe reports a new version; the probe is part of `models --probe` |
| Subscription terms | documented in `docs/PROVIDERS.md`: the app drives the operator's own signed-in CLI, as TownReporter does; no credential is copied or stored |
| Windows process trees | kill the tree (`taskkill /T /F`), with a test that orphans nothing |
| GPUI UI tests are slow | the page is asserted by the existing UIAutomation harness, plus `pfl-cli models` for the headless path |
| Scope creep into custom connections | explicitly out of scope; a second plan if wanted |
| A registry entry with no test | T1's consistency test fails the build |

## 7. What "done" will not mean

This plan does not make the product shippable. Signing, publishing, live paid verification and
interactive acceptance on each platform remain separate decisions for Scott. It makes the provider
choices **real, tested and reachable from the window**, and records exactly what is proven.
