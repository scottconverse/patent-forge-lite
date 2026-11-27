# Providers: how this build chooses a model

What exists, what it promises, and what it does not do yet. Every claim here is backed by a test or
an evidence directory; where something is missing it says so rather than leaving it implied.

## The shape

- **Registry** — `crates/pfl-workflow/src/registry.rs`. Pure: no I/O, no environment reads. Eight
  entries, each with a label, a detail line, a budget, and whether it is available. A new provider
  is an entry, not a new branch in five files.
- **Transports** — one per family: `cli.rs` runs the operator's own Claude, Codex and Gemini
  programs; `providers.rs` speaks Anthropic's API and any OpenAI-compatible `/v1` endpoint;
  `local_discovery.rs` finds servers on this machine.
- **Catalogue** — `catalog.rs` joins the registry, discovery and the CLI sign-in probes into one
  view. The Models page and `pfl-cli models` render that same view, so they cannot disagree.
- **Ladder** — `ladder.rs`: an order, a failure taxonomy, and a receipt.

## What each kind means

| Kind | Leaves the machine | Metered | Notes |
|---|---|---|---|
| `claude` / `codex` / `gemini` | yes | no | the operator's signed-in program; no credential is read, stored or logged |
| `anthropic` / `openai` | yes | yes | needs a key; the key lives in the session, never in the database |
| `local-lmstudio` / `local-ollama` / `local-llamacpp` | no | no | whatever the server has **loaded**, never a model loaded on its behalf |
| `replay` | no | no | recorded fixtures; cannot carry a real invention |

## The rules that are enforced, and where

**One consent rule** (`service.rs::data_rule_violation`) covers every kind, because the question is
not "is this Cloud" but "does this run leave the machine, and is this provider metered". A
subscription program runs locally but its prompt does not; a hosted API bills per token; a local
server does neither. Four tests cover the matrix by kind.

**Data never leaves by accident**: a run with real-invention data is refused for every off-machine
kind before the program is started.

**Never load a model**: discovery reports `loaded` where the server says so, and the model chosen
for a run is one the server already reports as loaded.

**One sign-in at a time**: two Codex device flows print two codes and only one works.

## Using it

- **Models page** — every provider with its availability *and the reason when it is not*. Choose one
  for every workflow, name a fallback, or choose Automatic, which clears the record rather than
  pinning something.
- **Command line** — `pfl-cli models` prints the catalogue; `pfl-cli models --set-surface
  stage-prior-art --provider claude --rank 0 --database … --project …` assigns one stage.
- **Automatic** — the ladder's order is the registry's rungs, filtered to what this machine can
  reach. Advance only on quota, unavailable, lapsed sign-in, timeout, no output or an unreadable
  answer; a content refusal or an unrecognised failure stops and is shown.

## What is not done yet

- **A per-stage run has not been observed.** The wiring is in: a run carries a `provider_plan`, the
  service takes a stage resolver, the session precomputes a stage→provider map, and every checkpoint
  records the provider that answered. But no run has been *seen* using a stage-level assignment,
  because no local model is loaded on this machine and a subscription run would spend the operator's
  quota. The cheap path to observe it is in `docs/HANDOFF-PROVIDERS.md`.
- **Hosted `openai`-compatible assignment by name.** `anthropic` builds from the session key, and a
  saved connection is usable as `conn-<id>`; there is no connection picker, and a connection whose
  key lives in the OS store is not yet resolved.
- **Per-run override** (the plan's T15) is not built: overrides are per project surface, not per run.
- **Gemini live call.** The transport is tested against its documented JSON; the live call is
  blocked on a Google sign-in or `GEMINI_API_KEY`.
- **Connection UI** (T16's remaining half): connections can be created by the CLI and used by a run,
  but not managed from the window.

## Evidence

- `crates/pfl-workflow/src/registry.rs` — 10 tests, including list/registry agreement.
- `crates/pfl-workflow/tests/openai_frames.rs` — 11 tests against a loopback stand-in.
- `crates/pfl-workflow/tests/claude_cli.rs`, `codex_cli.rs`, `gemini_cli.rs` — capture-derived
  fixtures, fake programs, and one live call each where a subscription exists.
- `crates/pfl-workflow/src/local_discovery.rs` — 6 tests; live probes recorded in the matrix.
- `crates/pfl-workflow/src/ladder.rs` — 6 tests, including quota-then-success with a receipt.
- `crates/pfl-storage/tests/assignments.rs` — assignments, connections, login rows.
- `docs/evidence/models/20261126T1130Z/` — the page as it ran on this machine.
- `crates/pfl-workflow/tests/login_manager.rs` — 5 tests: URL then finish, redacted failure, cancel,
  one attempt per provider, a late URL.
- `crates/pfl-workflow/src/ollama_supervision.rs` — 5 tests: a running server is never spawned, a
  spawn that never serves is a failure, one that comes up is reported started.
- `crates/pfl-domain/tests/checkpoint_provider.rs` — 2 tests: the provider field round-trips, and an
  older checkpoint without it still reads.
- `crates/pfl-desktop/tests/` — session state tests, including checkpoint ordering.