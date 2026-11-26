# Provider choice and release readiness: bounded implementation plan

**Direction: everything here is "must be done." The only excluded actions are tagging a release and
publishing it.** No item below may be dropped, deferred or reclassified as optional. If an item
cannot be completed, that is a defect to report, not a scope cut.

Status: replaced the earlier version of this plan on 2026-11-25 (owner direction).
Owner: Scott. Author: the implementation agent.

## 1. What is being built

For every stage of every workflow in PatentForgeLite, the operator chooses which model runs it, from
a list the application discovers itself:

- **local servers on this machine** — LM Studio, Ollama (including one this app starts if Ollama is
  installed but stopped), llama.cpp, vLLM, and any OpenAI-compatible server on a known address;
- **hosted APIs with a key** — Anthropic, and any OpenAI-compatible gateway (OpenRouter, DeepSeek,
  Groq, Together, OpenAI, Google's compatibility endpoint);
- **subscription CLIs already signed in** — `claude`, `codex`, `gemini`;
- **custom connections** the operator defines (name, base URL, key, model);
- **Automatic**, which probes a ladder and pins whichever rung answers, leaving a receipt.

Plus, for the product to be shippable: live provider verification, interactive acceptance on all
three platforms, a signed Windows installer, and a record that proves each claim.

## 2. Non-negotiable rules carried through every task

1. **No credential is read, stored or logged** by this application. Subscription access is the
   operator's own CLI, which owns its credential file and refresh; only a URL, a one-time code the
   human reads, and an exit status cross our boundary. `detail` is redacted before storage.
2. **Never load or unload a model.** Starting an installed-but-stopped server is allowed; loading,
   swapping or evicting a model is not.
3. **An off-machine provider never receives real invention data.** Synthetic only, enforced in
   `authorize`, with a test per provider kind.
4. **One open sign-in per provider.** Two device flows print two codes and only one works.
5. **Each task is one commit.** A second commit is allowed only to fix that task's tests. A third
   means stop and report.
6. **No tag, no publish, no release.** Everything else in this plan proceeds.

## 3. Tasks

Format: **What** · **How** · **Done when**.

### Phase A — Foundations

**T1. Provider registry, pure**
What: `crates/pfl-workflow/src/registry.rs` — `ProviderKind`, `ProviderEntry`, `PROVIDER_REGISTRY`,
`PICKER_PROVIDER_IDS`, `INTERNAL_PROVIDER_IDS`, `RETIRED_PROVIDER_IDS`, `KIND_BUDGETS`,
`providers_for(surface)`, `entry(id)`, `automatic_ladder()`.
How: surfaces are the 19 stages plus a workflow default. Entries carry model, label, detail, budget,
`offered_for`, `off_machine`, ladder rank, `requires_loaded_local_model`, `picks_loaded_local_model`,
and supported efforts. No `tokio`, no `std::env`: environment arrives through an injected `Env`.
Done when: tests prove every picker id has an entry; every id is in exactly one list; ladder order
equals ranks; budgets are non-zero and per-kind; a grep test fails if any provider id appears outside
`registry.rs`; a retired id resolves `None` and normalises to Automatic.

**T2. Widen provider identity without breaking stored runs**
What: `ProviderKind` gains `ClaudeCode`, `Codex`, `GeminiCli`, `OpenAi`; keeps `Local`, `Cloud`
(Anthropic), `Replay`. `ProviderSnapshot` gains `effort` and an optional `connection` (custom).
How: serde aliases so a 0.2.1 run (`"cloud"`, `"local"`, `"replay"`) still loads; an unknown kind
opens as `Replay` with a warning recorded on the run. `authorize` gains one arm per kind.
Done when: a captured 0.2.1 payload round-trips; the authorize matrix has a test per
(kind × authorization); an unknown kind opens with a warning and starts nothing.

**T3. Persistence**
What: `model_assignments(project_id, surface, rank, provider_id, model, effort)`,
`provider_settings(base_url, off_switch, budget overrides)`,
`api_connections(id, name, base_url, model, key_ref)` — the key itself never stored in plaintext and
never in the database, only referenced to the session.
How: one migration in the existing style; absent row means Automatic; rank 0 is the choice, 1–2 are
fallbacks.
Done when: CRUD tests pass; a pre-migration database opens reporting every surface unassigned;
deleting a project cascades; the schema version stays compatible with the released build's
positional `INSERT INTO runs` (the existing compatibility test still passes).

### Phase B — Transports

**T4. Anthropic HTTP (the existing Cloud path, made first-class)**
What: rename the internal kind to `Anthropic`, keep `PFL_CLOUD_ENDPOINT` for loopback verification,
add effort mapping where the API accepts it.
Done when: existing Cloud tests still pass unchanged; effort round-trips into the request body; an
evidence run against the loopback mock records the exact body.

**T5. OpenAI-compatible HTTP (extends the adapter already added)**
What: key handling, base-URL validation (http/https, host), `/v1/models` catalogue, usage capture,
404/401/429/5xx advice.
Done when: tests cover loopback (no key) and remote (auth header, never logged); a live run against
LM Studio and against Ollama's `/v1` is recorded; a malformed stream leaves nothing committed.

**T6. Claude Code CLI**
What: `providers/cli_claude.rs` — spawn `claude -p --output-format json`, `--model`, optional
`--effort`, `--no-session-persistence`, settings that disable CLAUDE.md/hooks/auto-memory/attribution.
How: argv copied from TownReporter's `buildClaudeCodeArgs`; stdin closed; stdout captured; stderr
redacted; kill the whole process tree on cancel; budgets from the entry.
Done when: a fixture CLI proves success, non-zero exit, timeout, mid-stream cancel and auth-failure;
an opt-in live run with the signed-in `claude` on this machine is recorded; a grep test proves no
credential reaches a log or the database.

**T7. Codex CLI**
What: `providers/cli_codex.rs`, same trait and guarantees.
How: first step is reading `ai-codex.server.ts` in full, then copying its argv and output parsing;
never plain `codex login` (fixed port 1455).
Done when: the same five fixture tests pass; an opt-in live run with `codex` is recorded; cancelling
orphans no process on Windows or POSIX.

**T8. Gemini CLI**
What: `providers/cli_gemini.rs`.
How: copied from `ai-gemini-cli.server.ts`; offered on every surface.
Done when: fixture tests pass; if `gemini` is absent here, install it from the vendor's documented
package and complete a live run — the task is not done until a real run is recorded.

**T9. Ollama native**
What: keep the existing Ollama provider, add catalogue listing with `loaded` where available.
Done when: tests cover `/api/tags`; a live listing against the Ollama on this machine is recorded.

### Phase C — Sign-in

**T10. Subscription sign-in**
What: `provider_logins` table and `start_login` / `poll_login` / `cancel_login` / `probe_login`;
argv `claude auth login --claudeai` and `codex login --device-auth`.
How: spawn detached, stdin closed; parse the authorize URL and the one-time code with the shapes
TownReporter's captured fixtures show; statuses `starting | awaiting_user | done | failed | expired |
cancelled`; one open attempt per provider; 10 min (Claude) / 15 min (Codex) expiry; `detail` redacted.
Done when: parsers pass against captured real output including ANSI, with the failure cases (prompt
line is not a URL, banner version is not a code, punctuation trimmed); a test asserts no
token-shaped string is stored; a real sign-in on this machine is completed end to end and recorded;
`probe_login` reports false when the CLI is absent.

### Phase D — Local runtime

**T11. Discovery**
What: probe `127.0.0.1:1234`, `:11434`, `:11435`, `:8080` plus configured overrides; models with
`loaded`, kind and embedding classification; "use whatever is loaded".
Done when: mock-server tests cover LM Studio, Ollama, a server answering neither, and loaded
selection; a live discovery against both servers on this machine is recorded; "use whatever is
loaded" resolves at call time and appears in the receipt.

**T12. Start an installed-but-stopped Ollama**
What: if `ollama` exists and nothing answers on 11434, start it, wait for readiness, and record that
this app started it (TownReporter's DEF-2 shape: its own supervision state, one writer).
How: never start if something else already answers; never stop a server the app did not start.
Done when: a test proves the app starts a stopped server, does not start one that answers, and does
not stop a server it did not own; the Models page shows which server this app started.

### Phase E — Selection

**T13. Automatic ladder, failover, receipts**
What: pure `plan_failover()` plus the executor; `RunReceipt { requested, actual, effort,
skipped[{rung, reason}] }` persisted on the snapshot.
Done when: each taxonomy entry has a test (quota/unavailable/login-lapse/timeout/no-output/unreadable
advance; refusal/unrecognised stop); an integration test with a quota-failing provider then a good one
asserts the receipt names the skip reason; a resumed run does not re-probe from the top.

**T14. Models page**
What: one row per registry entry with status (signed in / not installed / no answer / started by
this app), model list, sign-in button when an auth-shaped failure is detected, per-stage assignment
with fallbacks, effort control where supported.
Done when: the UI harness asserts one row per registry entry; choosing a model persists and a later
run requests it; Automatic pins nothing; the sign-in button appears only for auth-shaped failures;
screenshots recorded.

**T15. Per-run override**
What: choose a model for one run without changing the stored assignment; the run's receipt records
the override and its source.
Done when: a test proves the override is used once and the assignment is unchanged afterwards; the UI
shows the override as a distinct state; the receipt records `source: "override"`.

**T16. Custom API connections**
What: operator-defined connection (name, base URL, model, key) usable on any surface, key held in the
session only.
Done when: a connection can be created, used for a run and deleted; a test proves the key is absent
from the database and from every log; base-URL validation rejects non-HTTP(S) and credential-bearing
URLs.

### Phase F — Verification (each is a task; none may be skipped)

**T17. Live paid verification**
What: one real Anthropic run and one real OpenAI-compatible gateway run, with the operator's key,
recording request, response, usage and receipt.
How: the operator enters the key once in the Models page; the run uses synthetic data only.
Done when: both runs complete, their receipts name the actual model and usage, and the evidence
directory holds the transcript. **Dependency: the operator's key and consent to spend.**

**T18. Live PatentsView research**
What: one real PatentsView query with the operator's key.
Done when: a run returns references with publication numbers and the evidence records them.
**Dependency: the operator's PatentsView key.**

**T19. Interactive acceptance on all three platforms**
What: drive the real app on Windows (UIAutomation harness), on macOS (the Mac mini), and on Linux
(Xvfb with a captured screenshot plus the harness where it applies).
Done when: each platform has a recorded session showing the Models page, a chosen provider, a run
started and completed on the replay or local path, and the receipt; the Windows harness passes its
checks; the macOS and Linux sessions are committed as transcripts.
**Dependency: an Accessibility grant on the Mac mini for scripted UI, or the operator clicking
through; a Linux display is provided by Xvfb in CI.**

**T20. Installer, signing and upgrade proof**
What: rebuild the installer from the final code revision; run the Windows signing step with the
operator's certificate; record macOS as unsigned by policy (no Apple certificate exists); re-run the
upgrade proof against the signed installer; update the manifest and matrix.
Done when: `Get-AuthenticodeSignature` reports a valid signature from the operator's certificate; the
upgrade proof passes 12/12; the manifest hash matches the published-candidate file; CI is green.
**Dependency: the Windows signing credential and the operator's approval to use it.**

### Phase G — Record

**T21. Documentation, matrix and evidence**
What: `docs/PROVIDERS.md`, a manual section, matrix rows per provider with evidence, BUILD-EVIDENCE
entries, and the acceptance checker extended to refuse a provider row whose evidence path is missing.
Done when: the checker passes; every provider row cites an existing evidence directory or test; the
manual explains sign-in, local servers, custom connections and Automatic in operator language; CI is
green on all three platforms.

## 4. Sequence

```
T1 → T2 → T3 → { T4, T5, T6, T7, T8, T9 } → T10 → T11 → T12 → T13 → T14 → T15 → T16
   → T17 → T18 → T19 → T20 → T21
```

T4–T9 may be taken in any order. T17–T20 may overlap where they do not touch the same files.

## 5. Gates

Every task commit must pass, in this order:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p pfl-desktop --features native --all-targets -- -D warnings
cargo test --workspace
cargo test -p pfl-desktop --features native --lib
pwsh ./scripts/acceptance-check.ps1
```

and CI must be green on Windows x64, macOS arm64 and Linux x86_64 before the next task starts.
Because shipping code changes, the installer must be rebuilt whenever T20's freshness check reports
it is behind, and the upgrade proof re-run.

## 6. Dependencies on Scott (they block specific tasks, they do not remove them)

| Task | Needs |
|---|---|
| T10 | nothing from Scott — the CLIs are already signed in on this machine |
| T17 | one API key (Anthropic or a gateway) and consent to spend on two short runs |
| T18 | a PatentsView key |
| T19 | macOS: an Accessibility grant or a click-through; Windows and Linux need nothing |
| T20 | the Windows code-signing certificate and approval to sign |

If a dependency is not available when its task comes up, the task is reported as blocked with the
concrete reason and the plan stops there — it does not skip ahead and it does not mark anything pass.

## 7. Definition of done for the whole plan

- T1–T21 complete, each with the evidence named in its own "Done when".
- A provider can be chosen per stage, per run, or by Automatic; local servers, hosted APIs,
  subscriptions and custom connections all work and are reachable from the Models page.
- Live verification recorded for a paid API and for PatentsView.
- Interactive acceptance recorded on Windows, macOS and Linux.
- The Windows installer is signed, its hash is in the manifest, and the upgrade proof passes.
- CI green on all three platforms at the final commit.
- **No tag, no publish, no release.**

Those last three words are the only exclusion in this document.
