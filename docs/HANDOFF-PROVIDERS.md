# Provider choice: where this stands, and exactly what is next

A handoff for continuing `docs/PROVIDER-CHOICE-PLAN.md`. It states what is built and verified, what
is not, and the one change that unblocks the rest. The matrix in `docs/ACCEPTANCE.md` is the
authority for individual rows; this document is the shape.

## Built, tested, and evidenced

| Area | Where | Evidence |
|---|---|---|
| Registry, budgets, ladder rungs | `crates/pfl-workflow/src/registry.rs` | 10 tests |
| Provider identity + legacy compat | `crates/pfl-domain` | 3 tests, captured `cloud` payload |
| Assignments, connections, login rows | `crates/pfl-storage` | `tests/assignments.rs`, 13 tests |
| OpenAI-compatible transport | `crates/pfl-workflow/tests/openai_frames.rs` | 11 tests |
| Claude CLI transport | `tests/claude_cli.rs` | 10 tests + one live call |
| Codex CLI transport | `tests/codex_cli.rs` | 8 tests + one live call |
| Gemini CLI transport | `tests/gemini_cli.rs` | 6 tests; **live call blocked on sign-in** |
| Sign-in probe, parsing, manager | `src/cli_login.rs` | 5 + 5 tests; live probe on this machine |
| Local discovery + supervision | `src/local_discovery.rs`, `src/ollama_supervision.rs` | 6 + 5 tests; live probe |
| Ladder taxonomy + receipt | `src/ladder.rs` | 6 tests |
| Catalogue join | `src/catalog.rs` | 5 tests; live `pfl-cli models` |
| Models page | `native/mod.rs` | screenshot `docs/evidence/models/20261126T1130Z/`; choice, fallback and Automatic verified in the database |
| Consent matrix by kind | `src/service.rs::data_rule_violation` | 4 tests, all five off-machine kinds |

## Not built, and why

**1. A run uses one provider.** `WorkflowService` holds `provider: Arc<dyn Provider>`
(`service.rs:25`) and every run input records `self.provider.snapshot()` (lines 300, 433, 610).
`authorize` compares the run's snapshot to it (line 208). Consequences:

- a **stage-level** assignment is validated and stored but not honoured during a run;
- the ladder exists and is tested, but a run cannot *execute* a failover between rungs;
- T15 (per-run override) works only by rebuilding the whole service, which the session already does
  per run — it is not exposed as a per-run control.

The change that unblocks all three: let a run carry **more than one** provider snapshot (a list in
`RunInput`, validated on resume, with the receipt naming which rung answered), and let
`WorkflowService` take a resolver `Stage -> Arc<dyn Provider>` instead of a single provider. Default
that resolver to the current single provider so existing runs and resume compatibility are
unchanged.

**2. Hosted assignment.** `anthropic` and `openai` assignments fall back to the configured mode:
`assigned_provider` in `crates/pfl-desktop/src/session.rs` maps only subscriptions and local servers.
The keys live in the session (`cloud_key`) and in `api_connections.key_ref`; neither is plumbed into
a per-run provider yet.

**3. Custom connections (T16).** Storage and tests exist. A CLI command was attempted and
**reverted**: the CLI validates flags per command (`only live accepts --endpoint and --model`), and
the attempt died there. Start by reading that validator, then add `--plan`-style flags to it.

## Open rows you cannot close without Scott

- **Gemini live call** — run `gemini` once and complete Google's sign-in, or set `GEMINI_API_KEY`.
- **PatentsView live query** — needs a key.
- **A real re-authentication** — both CLIs are already signed in; re-auth opens a browser.
- **macOS artifact of the current revision** — an unsigned `.app` of an earlier revision exists;
  publishing any artifact is Scott's decision.
- **Windows signing** — needs the code-signing certificate.

## Verifying any change here

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p pfl-desktop --features native --all-targets -- -D warnings
cargo test --workspace
pwsh -NoProfile -File scripts/acceptance-check.ps1     # 13 checks, includes installer freshness
```

Then rebuild the installer (`target/native` release, ISCC), update `docs/RELEASE-MANIFEST.md`, and
run `scripts/upgrade-proof.ps1`. The freshness check fails if shipped code moves without that.

## Step 3: the session wiring, and its one trap

The four prerequisites are in (`RunInput.provider_plan`, `WorkflowService::new_with_stage_provider`,
`Checkpoint.provider`, `set_provider_plan`). What remains is `Request::Start` in
`crates/pfl-desktop/src/session.rs`.

**The trap:** `StageProvider` is a *synchronous* `Fn(Stage) -> Arc<dyn Provider>`, while building a
provider is *async* — a local kind needs discovery to choose the loaded model. A resolver therefore
cannot construct anything; it can only look up something already built.

So the wiring precomputes, before the run:

1. read the project's assignments (`store.assignments(&project)`);
2. for each stage-level rank-0 row, build its provider asynchronously and keep a
   `HashMap<Stage, Arc<dyn Provider>>`;
3. build the workflow-level provider exactly as `assigned_provider` does today;
4. `WorkflowService::new_with_stage_provider(store, workflow_provider.clone(), handle,
   move |stage| map.get(&stage).cloned().unwrap_or_else(|| workflow_provider.clone()))`;
5. `Arc::get_mut(&mut service).unwrap().set_provider_plan(plan)`, where `plan` is the distinct
   snapshots in rung order — **before** the service is shared. `Arc::get_mut` fails once a clone
   exists, so set it first, clone second.

Two consequences worth stating: a stage whose provider cannot be built falls back to the workflow
provider rather than failing the run, and the plan must contain every provider the resolver can
return, or `authorize` will (correctly) refuse the resume later.

## Observing a per-stage run (not yet done)

The wiring is in and compiles under both feature sets, but **no run has been observed using a
stage-level assignment**. The reason is concrete, not an oversight:

- a **local** provider must be *loaded* to answer; discovery currently reports `loaded: false` for
  every model on this machine (LM Studio 15, Ollama 36), and this application never loads one;
- a **subscription** provider would answer, but six stages of Claude or Codex is a real spend from
  the operator's quota, and this session's rule is not to make a run cost money to prove a
  mechanism.

To observe it, cheaply:

1. load one small model in the runtime the operator prefers (`ollama run <tiny>` — the operator's
   action, not the app's);
2. assign that server to one late stage:
   `pfl-cli models --set-surface stage-final-report --provider local-ollama --rank 0 --database … --project …`;
3. start a feasibility run with synthetic data;
4. read the run's checkpoints: every stage should record the workflow's provider, the final report
   should record the local one, and the plan stored on the run should contain both.

That converts the matrix row from "wired" to "observed". Until then the row says exactly what it is.