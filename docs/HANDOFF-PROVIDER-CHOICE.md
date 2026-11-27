# FULL HANDOFF — PatentForge Lite provider-choice session

You are taking over an autonomous session that ran 58 rounds and then **marked itself blocked**.
This document is written for you: a new AI session, with no memory of what happened. It contains the
mission, the constraints, what was built and by which commit, what was tried and failed, the traps
that cost the most time, exactly what remains, and — in the appendices — verbatim copies of every
document the work depends on.

Nothing here requires another file to be read first.

---

## 1. The mission, verbatim

> Implement `C:\Users\scott\Documents\Codex\PatentForgeLite\docs\PROVIDER-CHOICE-PLAN.md` in the
> PatentForgeLite repository: complete tasks T1–T21 (provider registry, transport adapters,
> subscription CLI sign-in, local runtime discovery and supervision, Automatic ladder with failover
> receipts, Models page, per-run override, custom connections, live provider and PatentsView
> verification, three-platform interactive acceptance, signed installer with upgrade proof, and
> documentation/matrix/evidence). Every item is must-do; **the only excluded actions are tagging and
> publishing**. Each task lands as a gated, CI-green commit with its own definition of done.

The plan itself is **Appendix A** — read it before judging any status below.

## 2. Constraints that are not negotiable

- **Never publish, tag, sign a release, or upload an artifact.** Publishing is the operator's
  decision. Do not "just" push a tag to test something.
- **Never contact a paid provider** without the operator supplying a key and consenting. The
  subscription CLIs (Claude, Codex) are the operator's own accounts: a live call spends their quota.
  Two live calls were made in this session (Claude, Codex) and that cost was accepted; do not repeat
  them casually.
- **Approval prompts are disabled.** Actions needing approval are rejected automatically; do not
  request sandbox escalation.
- **Do not disturb** the real PatentForge install, the DSH install, or the operator's local servers
  (LM Studio, Ollama). Starting an installed-but-stopped Ollama is allowed by the plan; stopping one
  is not.
- **Do not load or unload models.** The application never asks a server to load a model; "loaded" is
  something discovery observes, never something it causes.

## 3. State in one screen (verified at handoff)

| Fact | Value |
|---|---|
| Repository | `C:\Users\scott\Documents\Codex\PatentForgeLite` |
| Remote / branch | `scottconverse/patent-forge-lite`, `main` |
| Code revision | `594fdb7` was the revision this document described. The defect-closure work in §17 followed it; take the current HEAD from `git log` |
| CI at HEAD | green on windows-latest, macos-15, ubuntu-24.04 — run `45935134452` |
| Acceptance checker | `scripts\acceptance-check.ps1` → **13/13** |
| Matrix | 171 `pass`, 2 `blocked — harness`, 4 `blocked — authorization`, 1 `blocked — key`, 2 `not verified` |
| Tests | `cargo test --workspace` passes: **359 tests across 47 test binaries**, 0 failing (measured at `594fdb7`) |
| Installer | `dist\patent-forge-lite-0.2.1-x64.exe`, rebuilt at every code commit; upgrade proof 12/12 |
| Goal state | blocked (by this session), reason: the remaining plan tasks need operator credentials/access |

## 4. Do this first (in order)

1. Run the four gates in §6 and the acceptance checker. Confirm 13/13, and read `git status` rather than expecting a clean tree — this document may be untracked or modified. If either
   fails, stop and read §12 — you are probably about to repeat a known mistake.
2. `gh run list --repo scottconverse/patent-forge-lite --workflow native-manual.yml --limit 1` and
   confirm the newest run is green at HEAD.
3. Read Appendix A (the plan) and Appendix E (the matrix). Then enumerate the 3/4/1 blocked rows
   yourself; do not trust this document's memory of which they are.
4. Start with **§10.1 (`openai` assignment by name)** — it needs nothing from anyone.
5. Only then consider the operator-dependent items in §11.

## 5. Environment and paths

| Thing | Value |
|---|---|
| Session working directory | `C:\Users\scott\Desktop\Code\verdict-wt\merge-trial` — **not** the repo; pass `workdir` on every command |
| Shell | PowerShell 7 (`pwsh`) on Windows 11 |
| Rust | repository-pinned toolchain; `cargo` on PATH |
| Claude CLI | `C:\Users\scott\scoop\apps\nodejs22\current\bin\claude.cmd` (a POSIX `claude` script sits in the same directory — Windows cannot run it) |
| Codex CLI | `C:\Users\scott\scoop\shims\codex.exe` |
| Gemini CLI | `C:\Users\scott\scoop\apps\nodejs22\current\bin\gemini.cmd` (`@google/gemini-cli` 0.71.1, installed during this session, **not signed in**) |
| Local servers | LM Studio `http://127.0.0.1:1234`, Ollama `http://127.0.0.1:11434`; llama.cpp `:8080` absent |
| App data database | `%LOCALAPPDATA%\patent-forge-lite\projects.sqlite3` |
| Inno Setup | `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe` |
| Windows signing certificate | **absent** |
| Apple certificate / notarisation | **absent** — the macOS artifact is unsigned by policy |
| CI workflow | `.github/workflows/native-manual.yml`, triggered by pushing `main` |

**Always clear inherited `GIT_*` variables before any git command.** A polluted environment
(`GIT_CONFIG_COUNT` without keys) makes every git call fail with
`fatal: unable to parse command-line config`. It happened repeatedly; the checker clears them itself,
you should too.

## 6. The working method (this is what made progress possible)

Every change goes through this loop, in this order:

```powershell
Get-ChildItem Env: | Where-Object { $_.Name -like 'GIT_*' } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo clippy -p pfl-desktop --features native --all-targets --offline -- -D warnings
cargo test --workspace --offline
pwsh -NoProfile -File .\scripts\acceptance-check.ps1
```

- **`cargo build` is not a gate.** It does not compile integration tests and, without
  `--features native`, it does not compile the window at all. Twice a "clean build" hid a broken
  match. Always use the two clippy lines above.
- **Do not commit until all five are green**, and read the exit codes rather than the scrollback.
- **Commit, then rebuild the installer, then push.** Pushing `main` starts CI; do not push until the
  installer and the two documents agree (see §8).
- **Check CI before claiming a task done**, and if a job is cancelled with no log, re-run it — never
  "fix" code for a cancelled runner.

Installer cycle after any shipped-code change (`crates/**` outside `tests/`):

```powershell
Get-Process -Name 'pfl-desktop' -ErrorAction SilentlyContinue | Stop-Process -Force
$env:CARGO_TARGET_DIR='C:\Users\scott\Documents\Codex\PatentForgeLite\target\native'
cargo build -p pfl-desktop --features native --release --offline
Copy-Item .\target\native\release\pfl-desktop.exe .\target\release\pfl-desktop.exe -Force
Remove-Item Env:\CARGO_TARGET_DIR
& "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe" "/DAppVersion=0.2.1" .\packaging\windows\patent-forge-lite.iss
```

Then update the two documents **in the same chore commit**:

- `docs\RELEASE-MANIFEST.md`: `Revision` = the **code** commit (not the chore commit), plus the new
  installer SHA256 and byte size.
- `docs\ACCEPTANCE.md`: its `Revision` row must equal the manifest's revision (the checker compares
  them), plus any row whose status or evidence you changed, and the summary counts.

## 7. The story, phase by phase

This is what happened, so you can tell which parts are settled and which were hard-won.

**Phase A — foundations (T1–T5).** `ProviderKind` gained `ClaudeCode`, `Codex`, `GeminiCli`, `OpenAi`
beside `Replay`/`Local`/`Cloud` (Cloud kept for stored runs; unknown kinds deserialize to `Replay`
rather than failing). A registry of eight entries with per-transport budgets and ladder rungs, pure
and injectable. Storage: `model_assignments` (rank 0..2, cascade), `provider_settings`,
`api_connections` (**no key column**, only `key_ref`) and later `provider_logins`. Effort on the
call rather than the snapshot, so it cannot invalidate resume. An OpenAI-compatible transport with
11 frame-level tests.

**Phase B — transports (T6, T7).** Claude CLI (`-p --output-format json`, envelope found rather
than assumed because the CLI prints a warning before it) and Codex CLI (JSONL events; the answer is
the last `agent_message`, usage from `turn.completed`, a failed turn's nested JSON unwrapped). Both
verified with one live call each; both fixtures came from those captures. This phase produced two of
the hardest bugs: waiting on the child's **pipes** made a cancelled run take 29 seconds (wait on the
child, kill by pid), and a POSIX **process-group kill** cancelled a shared CI runner (kill the pid,
never the group). Linux also hit `ETXTBSY` for freshly written scripts — retry the spawn.

**Phase C — Gemini (T8, transport only).** Its prompt goes on **stdin**, because a Windows `.cmd`
cannot receive an argv value containing newlines ("batch file arguments are invalid"). The transport
is tested against its documented JSON; the **live call never happened** because the CLI is not
signed in. That is still open.

**Phase D — sign-in (T10).** Probe uses the CLIs' own `status` (`claude auth status` is JSON;
`codex login status` prints its sentence **on stderr** and exits 0 — measured, not assumed). Parsers
read the authorize URL from the sentence and Codex's device code while rejecting a version banner,
with ANSI stripped first. A `LoginManager` starts, polls and cancels one attempt per provider,
killing the tree on Windows. Launchers are resolved on PATH (npm installs put a POSIX `claude`
script, `claude.cmd` and `codex.exe` in one directory; guessing the extension failed twice).

**Phase E — local (T11, T12).** Discovery asks each server the question it can answer: LM Studio's
`/api/v0/models` reports `state` and `type`; Ollama's `/api/tags` plus `/api/ps` separate installed
from loaded; a plain OpenAI server is honestly `None`. "Use whatever is loaded" is the lowest loaded,
non-embedding id. Ollama supervision is a pure decision with injected probe/spawn: a running server is
never spawned, a spawn that never serves is a failure.

**Phase F — ladder (T13, and later T53/T54).** A taxonomy where quota, unavailable, lapsed sign-in,
timeout, no output and unreadable answers **advance**, and a content refusal or unknown failure
**stops**. It was a tested function nobody called for a while; later a `LadderProvider` made it
answer as one provider, and the session built that provider from a surface's ranks 0/1/2, so a stage
now really fails over mid-run.

**Phase G — the page and the choices (T14, T15, T16).** One catalogue join shared by the Models page
and `pfl-cli models`. The page shows every provider with the reason when unusable, and offers: choose
for all workflows, name a fallback, Automatic (which **clears** the record), a per-run override, a
per-stage choice for the final report, the assignment record, and saved connections (list and
create). Assignments persist, are addressable at workflow and stage grain, and drive runs. A saved
connection is usable as `conn-<id>`.

**Phase H — the structural chain (five rounds, two reverts).** This is the sequence that makes
per-stage provision real, and it went one step at a time because each step revealed the next gap:
1. `RunInput.provider_plan` (serde default; older payloads parse) and membership-based `authorize`;
2. `WorkflowService::new_with_stage_provider` + `StageProvider` (synchronous resolver);
3. `Checkpoint.provider` — which provider actually answered, `None` on legacy rows;
4. `set_provider_plan` — the plan was private, so no caller outside the crate could set it;
5. the session precomputes a stage→provider map (because building a provider can need **async**
   discovery while the resolver is **sync**), sets the plan before the `Arc` is shared, and passes
   the resolver.
Two rounds were lost here to bulk edits (see §12 item 3), and both were reverted rather than repaired.

**Phase I — record.** `docs\PROVIDERS.md` says what the layer does and does not do;
`docs\HANDOFF-PROVIDERS.md` records the structural traps; verification records live under
`docs\evidence\verify\`; the Models page and a choice were screenshotted into
`docs\evidence\models\20261126T1130Z\`.

## 8. The commit ledger

Every task commit is paired with a "chore: rebuild the installer…" commit that updates the manifest,
the matrix revision and the installer. **The code commit is the revision the manifest names.** The
full list, newest first, is Appendix F. Highlights:

| Commit | What |
|---|---|
| `0ef1462` … `c0d39cc` | T1–T4: identity, registry, persistence, effort |
| `f9f7943` | T5: OpenAI-compatible transport |
| `b753f1a` | T6: Claude CLI |
| `28cc878` | T7: Codex CLI, and the model default that actually answers |
| `c60fe5d` | T8 transport (live blocked) |
| `9317138`, `3a3df61` | T10: sign-in probe, parsers, manager |
| `a8f03a2` | T12: Ollama supervision |
| `030217f` | T13: taxonomy, receipt, execute |
| `37cc92a` | T11: discovery |
| `ad17495` | catalogue join |
| `1060014` | Models page |
| `aec026c`, `28413f1`, `329f81b`, `62e2a85`, `28d977d`, `2785d55`, `ab4db36` | assignment read/write, Automatic, fallback, override, report-stage, lists, connection form |
| `a78d8ed` | one consent rule for every kind |
| `6b611ba`, `48051ad`, `6b9e915`, `724cbfa` | assignment → run for local, subscription, anthropic, saved connection |
| `ce12f48`, `42b8d2b`, `28ff4db`, `03354e5`, `14f3459` | the structural chain |
| `1a2d4af`, `7b836a0` | `LadderProvider` and its wiring |
| `2cf58e9`, `081e544` | run-level tests: resolver-served run; two providers on two stages |
| `3d42a57` | session test for adding a connection |

## 9. Code map

```
crates\pfl-domain\src\lib.rs            kinds, Stage::slug, ProviderLogin, ModelAssignment,
                                        ApiConnection, RunInput.provider_plan (+ plan()/permits()),
                                        Checkpoint.provider
crates\pfl-storage\src\lib.rs           tables and Store methods (assignments, connections, logins)
crates\pfl-workflow\src\registry.rs     8 entries, budgets, rungs, Env (no I/O)
crates\pfl-workflow\src\cli.rs          Claude/Codex/Gemini providers; shared run_program
crates\pfl-workflow\src\cli_login.rs    probe, URL/code parsing, LoginManager
crates\pfl-workflow\src\local_discovery.rs   LM Studio / Ollama / llama.cpp
crates\pfl-workflow\src\ollama_supervision.rs  start-if-stopped
crates\pfl-workflow\src\ladder.rs       FailureClass/classify/execute/LadderProvider
crates\pfl-workflow\src\catalog.rs      the joined view
crates\pfl-workflow\src\service.rs      data_rule_violation, StageProvider, provider_plan,
                                        checkpoint provenance
crates\pfl-desktop\src\session.rs       requests/replies, assignment resolution, per-stage map,
                                        per-run override, connections, AddConnection
crates\pfl-desktop\src\native\mod.rs    the window and the Models page
crates\pfl-desktop\src\state.rs         Page enum (Models)
crates\pfl-cli\src\main.rs              `models`, `models --set-surface …`
```

Tests worth knowing: `crates\pfl-workflow\tests\{claude_cli,codex_cli,gemini_cli,openai_frames,
login_manager,workflow}.rs`, `crates\pfl-workflow\src\ladder.rs` (unit), `crates\pfl-domain\tests\
checkpoint_provider.rs`, `crates\pfl-storage\tests\{assignments,persistence}.rs`,
`crates\pfl-desktop\tests\{connections,session,state,…}.rs`.

## 10. Open work

### 10.1 `openai` assignment by name (small, no operator input)
An assignment resolves `claude`/`codex`/`gemini`/`anthropic` and saved connections as `conn-<id>`.
A bare `openai` falls back to the configured mode. Decide the rule (likely: the single saved
connection, or a configured base URL), implement it in
`crates\pfl-desktop\src\session.rs::provider_for_assignment`, and add a session test beside
`crates\pfl-desktop\tests\connections.rs`.

### 10.2 The connection form's click-through
`Request::AddConnection` is session-tested; the button binding is only visible in the diff. Round 56
failed to drive it with UI Automation (`models: False`, `edits: 0`). Try screenshot → coordinate
clicks instead of UIA name lookup, or accept the row's current wording, which claims no click.

### 10.3 A per-stage run with two *real* providers
Needs one loaded local model (`ollama run <tiny>` — the operator's action). The mechanism is proven
in-process by `081e544`.

### 10.4 The two `blocked — harness` rows
Not all are this plan's tasks. Enumerate them from Appendix E before judging them unfixable.

## 11. Operator-only (cannot be done without Scott)

| Task | Needs |
|---|---|
| T8 Gemini live call | `gemini` signed in to a Google account, or `GEMINI_API_KEY` |
| T18 PatentsView live query | a PatentsView API key |
| T19 three-platform interactive acceptance | hands on the Mac mini (Accessibility grant / click-through) and the Linux host |
| T20 signed installer | Windows code-signing certificate and approval to use it |

## 12. The minefield — every mistake this session made, and the rule

1. **Committed past failing gates** (`28ff4db`, `1ba794d`, and one more) — fixed in follow-ups.
   *Rule:* read the five exit codes before `git commit`.
2. **`cargo build` without `--features native`** hid a broken match twice.
   *Rule:* gate with `--features native --all-targets`.
3. **Bulk edits on large files** (rounds 29, 32, 47): a "replace first occurrence" hit a function
   parameter instead of a struct field; a brace-counting script inserted a field *outside* a literal;
   a line-based insert duplicated a field an earlier replace had already added. All reverted.
   *Rule:* read the exact region, **one** `edit` with exact text, build, repeat. Never mix an anchored
   replace with a line-based insert in one file, and re-read before the second attempt.
4. **Matrix/manifest revision drift** — the checker requires the matrix's revision to equal the
   manifest's; twice a ship script set it to the wrong commit, and once re-reading the file
   afterwards silently discarded the fix.
   *Rule:* set both at the end, from the manifest value, and re-read the file after writing.
5. **Count regex typos** in your own scripts (a missing `\|`) set a summary count to 0 while rows
   existed. The checker caught it — let the checker be the judge.
6. **Tests in shipped source files** (`#[cfg(test)]` inside `src/lib.rs`) fail the installer
   freshness check.
   *Rule:* tests go in `crates/*/tests/`; the freshness rule exempts `tests/` and `examples/`.
7. **POSIX process-group kill** (`kill -TERM -<pid>`) cancelled a shared CI runner.
   *Rule:* kill the pid only; Windows uses `taskkill /T` for the `.cmd` shim.
8. **Waiting on pipes instead of the child** made cancellation take 29 s.
   *Rule:* wait on the child, terminate by pid, drain afterwards.
9. **`ETXTBSY`** on Linux for just-written scripts. *Rule:* retry the spawn on `raw_os_error() == 26`.
10. **Windows `.cmd` + argv newlines** = "batch file arguments are invalid".
    *Rule:* multi-line prompts go on stdin.
11. **Shell quoting**: an apostrophe inside a single-quoted fixture broke a POSIX script.
    *Rule:* no apostrophes inside single-quoted shell strings.
12. **The CLI validates flags per command** ("only live accepts --endpoint and --model"). A
    `connections` command died there and was reverted.
    *Rule:* read `crates\pfl-cli\src\main.rs`'s argument validation first.
13. **macOS CI jobs sometimes cancel with no log.** Re-run the job; do not change code.
14. **Installer hashes are not reproducible** (Inno embeds a timestamp). A different hash is not a
    different product.
15. **An invented matrix status (`partial`) was rejected.** Only `pass`, `not verified`,
    `blocked — harness`, `blocked — hardware`, `blocked — key`, `blocked — authorization` exist.
16. **`docs\HANDOFF.md` must not restate the pass count.** The checker enforces it: that file points
    at the summary, it does not repeat it. This document is separate for that reason — do not merge
    it into `HANDOFF.md`.
17. **This session overwrote `docs\HANDOFF.md` once** by writing without reading first; it was
    restored with `git checkout`. *Rule:* read a file before overwriting it, especially in `docs/`.

18. **A stripped `PATH` in the launching session breaks tests that shell out through `cmd`.** An
    external review hit `claude_cli::a_program_that_never_answers_times_out` failing because the fake
    batch file could not find `ping`: the harness handed `cmd` an empty `PATH` (a 10,088-character
    `PATH` that `cmd` could not see). With the machine plus user `PATH` all ten tests passed.
    *Rule:* when a test that spawns a batch file fails only in one shell, compare `PATH` before touching
    the code; the project is not at fault.

## 13. What a released product still needs

1. **Code complete:** §10.1, the seven defects in §15.3, and anything T19 turns up.
2. **Verification complete:** T8, T18, T19, plus §10.2–10.4.
3. **Signing:** Windows installer signed with the operator's certificate; macOS unsigned by policy —
   decide whether to ship it at all.
4. **Version and notes:** bump beyond `0.2.1`, update the manifest and the matrix, write release notes.
5. **Final gate:** the five commands in §6, checker 13/13, and a green CI run at the exact commit to
   release.
6. **Publishing is the operator's call** and was excluded from this work. Nothing was tagged,
   published or released.

## 14. One-paragraph summary you can paste to anyone

The provider-choice plan is implemented and verified everywhere it can be without operator secrets:
eight providers in a pure registry, three CLI transports (Claude and Codex exercised live, Gemini
tested but not signed in), sign-in probing and a start/poll/cancel manager, local discovery that
distinguishes loaded from downloaded, an Ollama supervisor that only ever starts what is stopped, a
failure taxonomy and a ladder that now really fails over mid-run, one catalogue shared by the Models
page and the CLI, a page that can choose a provider, a fallback, Automatic, a per-run override, a
per-stage choice and custom connections, a single consent rule that keeps real inventions on the
machine, assignments that drive runs, and checkpoints that record which provider answered each stage.
171 matrix rows pass, the acceptance checker is 13/13, and CI is green on three platforms. What is
left needs a Gemini sign-in, a PatentsView key, a Mac and a Linux machine with hands, and a Windows
signing certificate — plus one small unwired feature (`openai` by name) and seven recorded code defects (§15.3).

---

## 15. Scope of this document, and its corrections

**Only five of the repository's twenty-five documents are embedded** (Appendices A-E). The rest
are inventoried in 15.1 and exist in the repo. An earlier verification of mine reported documents
as "embedded" when it had matched only their *filenames* appearing as references; that check was
wrong and the claim it supported was wrong with it.

**This is a handoff for the provider-choice work, not for the whole product.** The repository
carries a longer history (a 77-finding audit, the macOS transcript, the user manual, the API
contracts) that is summarised nowhere here. Taking over the product rather than this plan means
reading `docs\STATUS.md` and `docs\BUILD-EVIDENCE.md` too; the latter is ~128 KB and is not
reproduced here on purpose.

### 15.1 Every document in `docs\`, and whether it is inside this file

| Path | Size | Status |
| --- | --- | --- |
| `docs\ACCEPTANCE.md` | 37345 bytes | embedded (appendix) |
| `docs\API-CONTRACTS.md` | 4383 bytes | in the repo, not embedded |
| `docs\API.md` | 3430 bytes | in the repo, not embedded |
| `docs\ARCHITECTURE.md` | 9309 bytes | in the repo, not embedded |
| `docs\BUILD-EVIDENCE.md` | 128718 bytes | in the repo, not embedded |
| `docs\CLOUD-CONSENT-DESIGN.md` | 3155 bytes | in the repo, not embedded |
| `docs\CONTRACT-V0.md` | 3446 bytes | in the repo, not embedded |
| `docs\FIRST-SLICE-ACCEPTANCE.md` | 7310 bytes | in the repo, not embedded |
| `docs\HANDOFF-PROVIDER-CHOICE.md` | 108177 bytes | in the repo, not embedded |
| `docs\HANDOFF-PROVIDERS.md` | 6988 bytes | embedded (appendix) |
| `docs\HANDOFF.md` | 5647 bytes | in the repo, not embedded |
| `docs\INTERACTIVE-CHECKLIST.md` | 8964 bytes | in the repo, not embedded |
| `docs\PACKAGING.md` | 3397 bytes | in the repo, not embedded |
| `docs\PLATFORM-RUNBOOK.md` | 3842 bytes | in the repo, not embedded |
| `docs\PROVIDER-CHOICE-PLAN.md` | 15133 bytes | embedded (appendix) |
| `docs\PROVIDERS.md` | 5409 bytes | embedded (appendix) |
| `docs\REGENERATION.md` | 4591 bytes | in the repo, not embedded |
| `docs\RELEASE-MANIFEST.md` | 2916 bytes | embedded (appendix) |
| `docs\RELEASE-NOTES-0.2.1.md` | 2932 bytes | in the repo, not embedded |
| `docs\RELEASE-NOTES.md` | 6904 bytes | in the repo, not embedded |
| `docs\STATUS.md` | 7779 bytes | in the repo, not embedded |
| `docs\USER-MANUAL.md` | 26053 bytes | in the repo, not embedded |
| `docs\native-ui.md` | 13425 bytes | in the repo, not embedded |
| `docs\storage-and-migration.md` | 10479 bytes | in the repo, not embedded |
| `docs\workflow.md` | 8985 bytes | in the repo, not embedded |

### 15.2 What was verified after writing this file

- the five appendices each contain their source's head **and tail** (full containment, not a first-line match);
- every `docs\*.md` named in this file exists;
- code fences are balanced; the file is valid UTF-8 with no replacement characters;
- `scripts\acceptance-check.ps1` passes 13/13;
- the matrix counts were recomputed from the matrix itself: 171 pass, 2 blocked-harness,
  4 blocked-authorization, 1 blocked-key, 2 not verified;
- the full suite was run: 359 tests in 47 binaries, 0 failing;
- every 7-hex commit cited here appears in the ledger, and spot-checked subjects match;
- the ledger (Appendix F) runs from `0ef1462` (the first task commit) to `594fdb7`.

### 15.3 Corrections found by an independent review of this revision

The first version of this document repeated a matrix claim that contradicted another row in the
same matrix, and embedded a false absolute. Both are fixed at the source; this section records
them so the next reader knows the record was corrected rather than always true.

- **The matrix contradicted itself about per-stage provision.** One row passed "a run's stages can
  be served by different providers" (proven by a run-level test with in-process fakes) while
  another said "a run holds one provider, so stage entries are recorded but not yet honoured".
  The second row is now `not verified`, with the reason: no run has been observed *reading a
  stage-level assignment from the store*.
- **"Open defects: None. F-001 to F-077 are closed" was false at HEAD.** It is replaced in
  `docs\ACCEPTANCE.md` with the seven items an independent review reports, each with a location,
  and a statement that the `open defect` count of 0 is not evidence that the list is empty.
- **`scripts\cloud-consent-proof.ps1` could not run**: seven `Record` calls and no definition. It
  now defines `Record`, prints a line per check, and exits non-zero when a check fails.
- **The summary counts omitted the `not verified` category**, which exists.

None of those defects was fixed in this session: they are recorded, not closed. The mechanical
fixes are listed in Appendix A's Phase plan and in the review; treat them as work still to do.

## 16. Per-task status (the plan's own titles)

| Task | Status | Where / why |
| --- | --- | --- |
| T1 Provider registry, pure | done | `crates\pfl-workflow\src\registry.rs`, 10 tests |
| T2 Widen provider identity | done | `0ef1462`; legacy `{"kind":"cloud",...}` payload tested |
| T3 Persistence | done | `56f3396`; assignments, settings, connections, logins + tests |
| T4 Anthropic HTTP, first-class | **partial** | effort mapping and Cloud tests pass, but the plan's rename of the kind to `Anthropic` was never done (`ProviderKind::Cloud` survives), and no recorded loopback-body evidence run was found |
| T5 OpenAI-compatible HTTP | done | `f9f7943`; 11 tests in `tests\openai_frames.rs` |
| T6 Claude Code CLI | done | `b753f1a`; 10 tests + one live call |
| T7 Codex CLI | done | `28cc878`; 8 tests + one live call |
| T8 Gemini CLI | **partial** | transport + 6 tests (`c60fe5d`); **live call blocked on sign-in** |
| T9 Ollama native | done | `OllamaProvider` exists and is used when the endpoint is not OpenAI-flavoured; `/api/tags` covered by discovery tests; live listing recorded (36 models) |
| T10 Subscription sign-in | done | probe, parsers, `LoginManager`; a real re-auth is operator-only |
| T11 Discovery | done | `37cc92a`; live probe recorded |
| T12 Start a stopped Ollama | done | `a8f03a2`; decision tested; a running server is never touched |
| T13 Ladder, failover, receipts | done | `030217f` + `1a2d4af` + `7b836a0`; wired into runs |
| T14 Models page | done | `1060014` onward; all controls + screenshots |
| T15 Per-run override | done | `62e2a85` |
| T16 Custom API connections | done | `724cbfa`, `2785d55`, `ab4db36`, `3d42a57` |
| T17 Live paid verification | **blocked - authorization** | needs the operator's paid API key |
| T18 Live PatentsView research | **blocked - key** | needs a PatentsView key |
| T19 Interactive acceptance, 3 platforms | **blocked - authorization** | needs the Mac and Linux machines with hands |
| T20 Installer, signing, upgrade proof | **partial** | installer + 12/12 upgrade proof done; **signing blocked** (no certificate) |
| T21 Documentation, matrix, evidence | **done, with corrections** | `docs\PROVIDERS.md`, `HANDOFF-PROVIDERS.md`, this file, the matrix, `docs\evidence\verify\` — and the four corrections in 15.3 |

**How these statuses were derived:** from the evidence that exists (tests, commits, recorded runs),
not from a clause-by-clause audit of every "Done when". T4 and T9 were checked against their
criteria; T4 was corrected from `done` to `partial`. The other nineteen have not been re-audited
clause by clause; treat them as evidence-based, and re-read each task's "Done when" in Appendix A
before relying on it.

Open code, separate from the task list: **`openai` assignment by name** (§10.1) is unwired, and the seven defects recorded in §15.3 are open. Neither the matrix nor this file claims code complete.

---

---

## Appendix F - Commit ledger (verbatim from git, newest first)

Snapshot taken at `594fdb7`. The record corrections in §15.3 were committed after it and are not in this list; `git log` is the authority.

```text
594fdb7 chore: rebuild the installer at the connection session test
3d42a57 test: adding a connection through the session, saved and never contacted (T57)
da7bccd chore: rebuild the installer at the connection form
ab4db36 feat(desktop): add a connection from the Models page (T55)
d99a6cb chore: rebuild the installer at the ladder wiring
7b836a0 feat(session): an assignment's fallbacks become the run's ladder (T54)
4b7ac7a docs: the matrix names the manifest's revision again
8aef8eb chore: rebuild the installer at the ladder provider
1a2d4af feat: the ladder can answer as one provider, so a stage fails over (T53)
3767b53 chore: rebuild the installer at the two-provider test
081e544 test: two stages answered by two providers, both recorded (T52)
7727874 docs: the matrix names the manifest's revision
ee2b79c docs: the matrix names this revision
71c350c chore: rebuild the installer at the resolver test
2cf58e9 test: a resolver-served run names the answering provider on every stage
363d334 docs: the evidence list names every test file that now exists (T50)
b256645 docs: the harness count is three, not zero
c76388b chore: rebuild the installer at the stage control
28d977d feat(desktop): name a provider for the final report alone (T49)
95b14ef chore: rebuild the installer at the connections panel
2785d55 feat(desktop): the Models page lists saved connections (T48)
56d6056 chore: rebuild the installer at the assignment list
91ef396 feat(desktop): the Models page lists the project's assignments (T46)
207c38a chore: rebuild the installer at the per-run override
62e2a85 feat: a provider can be named for the next run only (T15)
342b858 docs: the provider layer's gaps, as they are now (T44)
cafe2fc chore: rebuild the installer at the connection-backed assignment
724cbfa feat(session): a saved connection can serve a run, addressed by its id
076bdb8 chore: rebuild the installer at the hosted assignment
6b9e915 feat(session): an anthropic assignment builds the hosted provider from the session key
14a27b3 docs: say exactly what observing a per-stage run needs, and why it has not been done (T40)
ed61148 docs: the wiring row's status is a plain pass
11ed836 chore: rebuild the installer at the per-stage wiring
14f3459 feat(session): a run''s stages can be served by the providers its assignments name
1dcf1ea docs: verification record at this revision (T38)
ea1ba19 docs: record the sync-resolver/async-provider trap before wiring step 3 (T37)
0417756 chore: rebuild the installer at the plan setter
03354e5 feat(workflow): the provider plan can be declared on a service (T36)
190c2fe chore: rebuild the installer at the service provider plan
ec26147 feat(workflow): a service's runs carry the provider plan it was given (T35)
823bf5e test: move the checkpoint tests to an integration file so they do not touch shipped code
649cce4 test: the checkpoint's provider field reads and round-trips (T34)
1917f2d fix: the last two checkpoint literals, and the harness count
bccec0e chore: rebuild the installer at the checkpoint provider
28ff4db feat(domain): a checkpoint records the provider that answered it (T33)
8da5d83 docs: the resolver row says blocked, not an invented status
d835a7d chore: rebuild the installer at the stage resolver
42b8d2b feat(workflow): a service can be given a per-stage provider resolver (step 2)
e679494 chore: rebuild the installer at the provider plan
ce12f48 feat(domain): a run can carry the provider plan behind its choice (step 1 of the structural change)
b62a1d4 docs: hand off the provider work with the exact next change (T27)
1d20d6b chore: rebuild the installer at the Automatic order
ebfd1b3 feat(cli): the catalogue says what Automatic would try (T26)
ff91189 docs: record a verification pass at this revision (T21)
0c4a082 chore: rebuild the installer at the Ollama supervision
a8f03a2 feat: start an installed-but-stopped Ollama, and never touch one already running (T23)
48dc31f docs: say what the provider layer does, and what it does not (T21)
8d0c5fb chore: rebuild the installer at the per-stage assignment
7de3895 feat(cli): assign a provider to one stage from the command line (T21)
2acad78 chore: rebuild the installer at the fallback control
329f81b feat(desktop): a provider can be named as the fallback (T20)
58b7e36 chore: rebuild the installer at the subscription assignment
48051ad feat(session): a subscription assignment builds the program a run uses (T19)
5463b8d chore: rebuild the installer at the consent matrix
a78d8ed feat(workflow): one consent rule covers every provider kind (T18)
00dc168 chore: rebuild the installer at the assignment wiring
6b611ba feat(session): a local assignment replaces the provider a run uses (T14, partial)
784bbed chore: rebuild the installer at the Automatic control and record it
28413f1 feat(desktop): Automatic is a reachable choice, and it clears the record (T14)
02324b4 docs: the harness count follows the row that became a pass
9e0f820 chore: rebuild the installer at the provider choice and record it
da6bcc5 feat(desktop): choose a provider from the Models page (T14)
e329266 chore: rebuild the installer at the assignment path and record what it proves
aec026c feat(session): read and write model assignments (T14)
9ec59d4 docs: file the Models page screenshot as evidence in its own directory
8efb717 chore: rebuild the installer at the Models page and record the screenshot
1060014 feat(desktop): the Models page (T14)
23e3676 chore: rebuild the installer at the catalogue reply
e59f4be fix(desktop): hold the catalogue reply so the window matches on it exhaustively
529bcde chore: rebuild the installer at the catalogue request
1ba794d feat(session): the window can ask for the model catalogue (T14, plumbing)
4451aed chore: rebuild the installer at the catalogue commit and record it
ad17495 feat: one model catalogue for the window and the command line (T14, foundation)
9e28da6 chore: rebuild the installer at T13 and record the ladder
030217f feat: the Automatic ladder, its failure taxonomy and the run receipt (T13)
c9fb977 chore: rebuild the installer at T11 and record discovery
37cc92a feat: discover the local servers and what they have loaded (T11)
e0b9c8b chore: rebuild the installer at the sign-in fixes
99df4e8 fix: use the pid on POSIX and retry a briefly-busy sign-in program
da86627 test: quote-safe fake output (an apostrophe inside single quotes is a shell syntax error)
1a6c070 test: a fast fake may finish before the first poll; assert the URL instead
35b9d9d chore: rebuild the installer at the T10 revision and record the sign-in manager
3a3df61 feat: start, poll and cancel a sign-in (T10)
5c05224 chore: rebuild the installer at the T10 core revision and record what it proves
9317138 feat: ask the installed CLIs whether they are signed in, and record login attempts (T10, partial)
a2c3706 chore: rebuild the installer at the T8 revision and record the Gemini blocker
c60fe5d feat(workflow): run the operator's own Gemini CLI, with its live check blocked on sign-in (T8, partial)
04102d6 chore: rebuild the installer after the ETXTBSY retry
f104312 fix(workflow): retry a CLI that is briefly ETXTBSY after being written
8dbb9a0 chore: rebuild the installer at the T7 revision and record the live Codex call
28cc878 feat(workflow): run the operator's own Codex CLI (T7)
6785495 chore: rebuild the installer after the pid-only termination
044d4ac fix(workflow): terminate the CLI by its own pid, never its process group
5159a62 test: show the CLI error on failure; a test change no longer demands an installer rebuild
bf9443b chore: rebuild the installer at the tokio process_group fix
70c8e40 fix(workflow): use tokio's own process_group so the unix-only trait import is not unused
697a76e chore: rebuild the installer at the process-group fix
430b8f6 fix(workflow): give the CLI its own process group so the POSIX kill cannot reach the app
ee0989e chore: rebuild the installer at the T6 revision and record the live CLI call
b753f1a feat(workflow): run the operator's own Claude CLI (T6)
53224c0 chore: rebuild the installer at the T4 revision and record effort
c0d39cc feat(workflow): effort reaches the Anthropic request as a thinking budget (T4)
9eac1cd chore: rebuild the installer at the T5 revision and record the adapter's tests
f9f7943 test(workflow): pin the OpenAI-compatible request path (T5)
fed5ebb docs: add the T1-T3 rows the matrix was missing
046ec41 chore: rebuild the installer at the T3 revision and record it
56f3396 feat(storage): assignments, provider settings and operator connections (T3)
1de2cbb chore: rebuild the installer at the registry commit and record T1/T2
63e6e98 feat(workflow): one pure provider registry
0ef1462 feat(domain): widen the provider identity, and never fail to read a stored kind
```

---

## Appendix A - The plan (verbatim)

Source: `docs\PROVIDER-CHOICE-PLAN.md` (copied verbatim, 15133 bytes).

```markdown
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
```

---

## Appendix B - The provider layer (verbatim)

Source: `docs\PROVIDERS.md` (copied verbatim, 5409 bytes).

```markdown
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
```

---

## Appendix C - Earlier structural handoff (verbatim)

Source: `docs\HANDOFF-PROVIDERS.md` (copied verbatim, 6988 bytes).

```markdown
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
```

---

## Appendix D - Release manifest (verbatim)

Source: `docs\RELEASE-MANIFEST.md` (copied verbatim, 2916 bytes).

```markdown
# Release manifest

Built from one revision. Every hash below was computed on this machine; re-run the
commands in *Reproducing* to check them.

| Field | Value |
| --- | --- |
| Revision | `3d42a57` |
| Branch | `main` |
| Candidate version | `0.2.1` |
| Licence | MIT (application); prompts CC BY-SA 4.0 |

## Artifacts

| Platform | File | Version | SHA-256 | Bytes | Signed |
| --- | --- | --- | --- | --- | --- |
| Windows x64 | `patent-forge-lite-0.2.1-x64.exe` | 0.2.1 release candidate | `e06101dde067adb3cf3076841727c504dbcc7ba6aedab59f351a00f6ad5124c0` | 12348586 | **no** |
| Windows x64 | `patent-forge-lite-0.1.0-x64.exe` | 0.1.0 released artifact, unchanged | `624f731cea661176ae9989bbba6234ee633af3d8b49314e0c151307b340c7f37` | 7,837,495 | **no** (the signed copy is `dist/signed/`) |
| Linux x86_64 | `patent-forge-lite-v0.2.1-linux-x64.tar.gz` | 0.2.1 release candidate, built in CI from `3535022` | `7e9046edaa7a112a6915ecdd19423ff7d2817cefe54a9ef3aba66ef3fdcd3a07` | not recorded | **yes** (CI only, no signing key involved) |

macOS and Linux now have artifacts, both built away from this Windows machine: Linux in CI
(tarball above, launched under Xvfb), macOS on the Mac mini (`docs/evidence/macos/20261125T023619Z/`,
unsigned because there is no Apple certificate). What neither has is a human using the window.

## One installer, two profiles

The Windows installer carries both profiles. The operator chooses Full or Lean at first
run; Lean refuses a local runtime and Full offers both a local runtime and the paid Cloud
provider. There is no separate Lean package.

## Version skew, stated plainly

The installer is named **0.2.1** from `AppVersion`
(`ISCC /DAppVersion=0.2.1`), and `crates/*/Cargo.toml` declares `0.2.1` too, so the artifact name and the crate version agree. The v0.1.0
release and tag are kept as an accepted decision, and re-issuing an artifact called 0.1.0
from a later revision would silently replace a published file. Choosing the next release
version, and bumping the crates to match, is Scott's decision; until it is made this
manifest is the record of the skew rather than a claim that it does not exist.

## Signature status

Nothing in this candidate is signed or notarised. The signed v0.1.0 copy remains at
`dist/signed/patent-forge-lite-0.1.0-x64.exe` and is unaffected. Signing a new revision
requires approval.

## Reproducing

```sh
cargo build -p pfl-desktop --features native --release
cp target/native/release/pfl-desktop.exe target/release/pfl-desktop.exe
"%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" /DAppVersion=0.2.1 packaging\windows\patent-forge-lite.iss
Get-FileHash dist\patent-forge-lite-0.2.1-x64.exe -Algorithm SHA256
```

The build is not bit-reproducible across machines (Rust does not promise that), so the
hash identifies *this* artifact, not a recipe that will always produce it.
```

---

## Appendix E - Acceptance matrix (verbatim)

Source: `docs\ACCEPTANCE.md` (copied verbatim, 37345 bytes).

```markdown
# Acceptance

What this build does, and what proves it. One row per requirement, grouped by phase.

**Evidence classes are not interchangeable.** `unit` is a test; `run` is a command whose
output is recorded; `installed` is the packaged application driven on this machine;
`mock` means a loopback stand-in stood in for a provider; `CI` is a build on a hosted
runner. A row is `pass` only when the evidence named in it exists in this repository.
Anything installed writes its JSON and, where relevant, its request log into
`docs/evidence/<area>/<timestamp>/`.

| Field | Value |
| --- | --- |
| Revision | `3d42a57` |
| Candidate version | 0.2.1 — declared in `Cargo.toml` and named by the installer, so there is one version, not an override |
| Published release | v0.1.0 only; this revision is **unpublished and unsigned** |

## Summary

| Status | Count |
| --- | --- |
| pass | 171 |
| not verified | 2 |
| blocked — hardware | 0 |
| blocked — key | 1 |
| blocked — authorization | 4 |
| blocked — harness | 2 |
| open defect | 0 |

## Phase A — Cloud correctness and consent

| Item | Evidence | Result |
| --- | --- | --- |
| Split UTF-8 across SSE chunks reassembles; invalid UTF-8 is an error, not U+FFFD | unit, 3 tests including the old defect | pass |
| Terminal frame required; a stream without one is `Truncated` | unit, installed run | pass |
| Stop-reason allowlist (`stop` / `end_turn` / `stop_sequence`) | unit | pass |
| Output-limit stop becomes `Truncated` with no report | unit, installed run | pass |
| Malformed or mid-frame stream is an error | unit | pass |
| Cloud mode at the session layer; key held in memory for the session only | unit, session tests | pass |
| Cloud start refused without explicit paid approval | unit, session tests | pass |
| The Cloud output cap clears the largest prompt target, and a limit stop names the limit | unit (`providers`) | pass — cap 16,000 vs 5,000-word target |
| Research survives a mode change: an opted-in run still queries after the service is rebuilt | integration (`research_consent`) | pass — 2 queries across a mode change |
| Findings in a package are those of the exported revisions; the manifest records their run | integration (`revisions`) + unit (`matching_findings`) | pass |
| Regenerate and resume carry a fresh authorization: a Cloud regeneration without current paid approval is refused and sends nothing; with it, it runs | integration (`cloud_consent`) | pass — 0 requests refused, then reaching the provider |
| Paid approval control present in the window; run refused without it | installed | pass |
| Declining sends zero requests; approval reaches the provider | installed + mock | **blocked — harness** — the scripted decline cannot be demonstrated: pressing Start through this script never reaches the handler, while the same control starts runs in `installed-local-proof` (5/5 at this revision). Its three run-path assertions were removed rather than left passing vacuously; the gate itself is proven by the session tests, and the earlier 7/7 evidence remains in `docs/evidence/cloud-consent/`. |
| No fallback: a failed Cloud call makes exactly one request and leaves no checkpoint | unit, `consent.rs` | pass |
| Loopback-only verification hook; a non-loopback Cloud endpoint is refused | unit | pass |
| Lean refuses a local runtime with a clear message | unit, `consent.rs` | pass |
| Lean permits Cloud past the profile gate (fails at the provider, not the gate) | unit, loopback 503 | pass |
| Masked key field: the mask preserves byte offsets, never paints the secret, and neither copies nor cuts | unit (`masking_tests`, compiled and run in CI with `--features native`) | pass — F-014 |
| Configuring Cloud answers without contacting a provider; an empty key is refused | unit (`cloud_probe`) | pass |
| Whether a run may start is one pure decision with text for every refusal, tested in order | unit (`gate_tests`, 5 tests) | pass |
| Both Word builders share one paragraph builder, and the package HTML carries the report's CSP and `lang` | unit + code | pass — F-068 |
| The rail is a vertical tab list, the page area is a TabPanel, workflow tabs answer Enter/Space, and Quit is a labelled focusable button | installed harness (window measure) + code | pass — F-070 |
| The installer offers no all-users override, and the matrix has no empty table | review | pass — F-077 |
| No package carries `docs/evidence`, and the proof scripts write placeholder paths instead of the operator profile | run + code | pass — F-072 |
| The release workflow is read-only by default, grants write only to the publish job, and pins every action to a commit SHA | review (`release.yml`) | pass — F-073 |
| The macOS plist is a template filled from the release version, not a hard-coded 0.1.0 | code (`bundle.sh`) | pass — F-071 |
| The CSV carries a UTF-8 BOM, a publication-number column and the IDS vocabulary; only PatentsView records are marked verified | unit (`package`, `research_http`) | pass — F-067 |
| A start refusal names the one control that is missing | unit (`gate_tests`) | pass — F-069 |
| The revision doc comment sits on the revision type, the metadata fence outgrows any backtick run, every note line is quoted, and cells strip carriage returns | unit + code | pass — F-075, F-076 |
| A revision must be anchored to a run of its own project and kind, a repeated revision id is idempotent, and a supplied regeneration base is validated | unit (`persistence`, `workflow`) | pass — F-064 |
| An imported legacy run that claims completion without every stage is Interrupted, and a NULL completion time falls back to the start | unit (`migration`) | pass — F-065 |
| The viewer clamps long hash runs, requires a space after `#`, and never lets an unclosed fence swallow later sections | unit (`block_edge_tests`) | pass — F-060 |
| An application report without claims text says so instead of printing the figures section as claims | unit + code | pass — F-066 |
| CLI: one command list drives both acceptance and usage; a boolean flag works anywhere; the generic research endpoint is loopback-only | run (`pfl-cli`) + unit | pass — F-059 |
| An API internal error returns a fixed sentence and logs the detail locally | unit + code | pass — F-062 |
| The import preview and the receipt report the same preserved count, and preview queries escape the table name | unit (`migration`) | pass — F-063 |
| A pasted key is trimmed before it is stored | unit (`session`) | pass — F-061 |
| The PatentsView body read is cancellable, the Cloud frame buffer is capped at 2 MiB, and 401/403/429/5xx read differently | unit + integration (`cloud_frames`) | pass — F-052, F-053 |
| The Lean-profile rule lives in `authorize`, so start, resume, rerun and regenerate all honour it | unit (`consent`) + code | pass — F-054 |
| A request carrying a key prints `[redacted]` under `Debug`; clearing the key drops the Cloud provider; the key-never-in-database scans read every file in the directory, WAL included | unit (`session`, `cloud_consent`, `research_consent`) | pass — F-055, F-056, F-057 |
| Package references come only from a completed feasibility run | unit + code | pass — F-058 |
| The Cloud key never reaches the persisted database | integration (`cloud_consent`, byte scan proven against a known value) | pass |
| The workflow, consent and storage documents describe the shipped behaviour: per-run paid approval, opt-in research, per-workflow checkpoints and document revisions | review | pass — F-046 |
| Import receipts and the storage document say credentials are redacted and must be re-entered | unit + review | pass — F-047 |
| Every HTTP write path has a test: create, intake save, start, download-token, resume and cancel | integration (`api`) | pass — F-048 |
| `inspect` opens read-only and refuses a missing database; `settings` takes the window's profile lock | unit + code | pass — F-018 |
| The upgrade proof refuses to run where Patent Forge Lite is really installed, installs without shortcuts, and removes its test install on failure as well as success | run (`upgrade-proof`) | pass — F-051 |
| The schema change keeps `runs` at five columns and the counter in `run_sequences`, so a rolled-back v0.1.0 build both opens the workspace and can still insert a run | unit (`persistence`, positional insert) + installed rollback launch | pass — F-050, rollback-write regression |
| Revisions are versioned per kind, listed in order and readable by id; an empty revision is refused | unit (`persistence`) | pass — F-035 |
| The import preview reports counts and is read-only; a missing source is refused | unit (`migration`) | pass — F-035 |
| A database with an unknown schema version is refused; a fresh import into a non-empty store is refused | unit (`persistence`, `migration`) | pass — F-035 |
| A streamed delta advances a sequence column and leaves the run payload byte-identical; event reads are paged | unit (`persistence`) + schema v2 | pass — F-032 |
| A v1 database upgrades in place to v2 when opened | unit + reopen paths | pass |
| Storage refuses a checkpoint whose stage belongs to another workflow | unit (`persistence`) | pass — F-033 |
| A legacy row storing null for an optional context field imports instead of rolling back | integration (`migration`) | pass — F-030 |
| Credential redaction covers provider-named keys, private keys, credentials, auth headers and URL userinfo; a bare key column is not redacted | unit (`credential_tests`) | pass — F-031 |
| A gapped legacy run is refused with a named reason and nothing partial is imported | integration (`migration`) | pass — F-034 |
| The paid gate refuses with a real Cloud provider, sends nothing declined, and reaches the stand-in when approved | integration (`workflow`) | pass — F-028 |
| Cloud stream faults are each refused: error frame, malformed frame, mid-stream end, output-limit stop, and a redirect that is not followed | integration (`cloud_frames`, 5 tests) | pass — F-028 |
| A save refused because a request is in flight says so, and every button action repaints so the notice is visible | unit (`gate_tests`) + code | pass — F-029 |
| CI runs the gate on every push and pull request, and a release tag cannot publish untested code | workflow config (`native-manual.yml`, `release.yml` verify job) | pass — F-036 |
| The acceptance check compares the matrix revision and hashes with the release manifest, refuses a restated HANDOFF count, and scans the reader-facing documents | run (`acceptance-check`, 10 checks) | pass — F-038 |
| The user manual, README, STATUS and the published page describe this build: Cloud selectable, migration implemented-but-not-enabled, unsigned candidate, revisions and regeneration documented | review | pass — F-040, F-041, F-042, F-044 |
| The revision diff trims the common prefix and suffix and caps the table, reporting an approximate comparison instead of allocating | unit (`bounds_tests`) | pass — F-022 |
| References reach the model with title, assignee and dates, under the heading the prompt acts on; the prompt version records the change | unit (`reference_tests`) + `providers` | pass — F-027 |
| The claim validator is bounded: a huge range cannot hang it and a claim number beyond the maximum is reported | unit (`parser_tests`) | pass — F-023 |
| The claim parser treats only `N. ` as a claim heading, folds wrapped lines, uses word boundaries, and parses `or`/`to`/dash ranges plus multiple dependencies | unit (`parser_tests`) | pass — F-024 |
| Word parts contain no XML-illegal control characters; parsed from `word/document.xml` after an export with an escape and a form feed | unit (`package`) | pass — F-025 |
| Only the leading run of headings is stripped, so a body line starting with `#` survives | unit (`heading_tests`) | pass — F-026 |
| A package is written only into an empty directory, so an existing package is never overwritten or left stale | unit (`write_package`) | pass — F-020 |
| CSV cells cannot start a spreadsheet formula | unit | pass — F-021 |
| The viewer keeps `snake_case` and `2 * 3`, stripping emphasis only at a line start | unit (`marker_tests`) | pass — F-019 |
| API download tokens are random and single-use, accepted on the export route only; a token-less server answers a loopback Host only | integration (`api`) | pass — F-016, F-017 |
| `serve` refuses a routable bind without `PFL_API_TOKEN` | unit + code | pass — F-016 |
| The PatentsView client follows no redirects, uses no proxy, validates its endpoint, and sends the key only to the configured host | unit + code | pass — F-007 |
| Real-invention data is refused with research, so text never leaves the machine | integration (`research_run`) | pass — refused start makes zero queries |
| An error-shaped PatentsView response fails the run instead of recording an empty set; a repeated idempotency key does not query again | integration (`research_run`) | pass |
| Every start and reconfigure path shares one idle guard; a second run cannot start beside the first | integration (`cloud_consent`) + code | pass |
| The PatentsView key never reaches the persisted database; an opted-in run makes exactly one query and records the reference | integration (`research_consent`) | pass |
| Cloud decline and approval through the session actor: declined run sends nothing, approved run completes with one request per stage | integration (`cloud_consent`), loopback SSE stand-in | pass — 0 requests declined, 6 approved |
| Cost line states *unknown* plus the rates, never a fabricated total | unit, `cost.rs` | pass |
| Cost line displayed in the window before a paid run | installed + mock, 7/7 twice | pass — the consent proof asserts `3.00 input / 15.00 output` |

## Phase B — a product a person can use

| Item | Evidence | Result |
| --- | --- | --- |
| Revisions versioned and anchored to immutable runs | integration | pass |
| Compliance reviews a selected revision rather than the latest | integration | pass |
| A selection from another project is refused, naming the reason | unit (`revisions`, cross-project) | pass |
| Claim structure validation (numbering, dependencies, references) | unit, CLI run | pass |
| Edit → validate → save → package chain | run | pass |
| `SaveRevision` / `ListRevisions` session commands, anchored to the latest completed run | unit, reply-draining helper | pass |
| Editing surface: revision field, Load run text, Save revision, List revisions | installed, harness 38/38 | pass |
| Editing end to end: load text, save v1, edit, save v2, both listed | installed, 4/4 twice | pass — `docs/evidence/editing/` |
| Deterministic line diff (LCS) | unit, 6 tests | pass |
| `pfl-cli diff` between two stored revisions | run | pass |
| Revision rows with a compare action and an inline diff | installed, 5/5 twice | pass — announces `Diff v1 to v2: 1 added, 1 removed` |
| Select revisions for compliance from the window; the run records them | installed, 8/8 twice; session test for the recorded id | pass — new run `v5 · compliance, Completed` |
| Export selected revisions (Markdown, HTML, Word, CSV, manifest) | installed, 11/11 twice | pass — six files written and listed |
| Word export byte-stability | two separate processes | pass — identical `claims.docx` and every other file |
| Regeneration semantics and plan validation | doc + unit, 5 tests | pass — `docs/REGENERATION.md` |
| Execute "regenerate from here onward"; earlier stages byte-identical | unit + session (`regenerate_session`) + installed, 12/12 twice | pass — v5 regenerated as v6, Completed |
| An interrupted regeneration resumes without re-committing its carried prefix | integration (`regenerate_resume`) | pass — F-010 |
| `pfl-cli revisions list` (id, kind, version, base run, note) | run | pass |

## Phase C — research

| Item | Evidence | Result |
| --- | --- | --- |
| References supplied to feasibility reach the dependent workflows | integration | pass |
| IDS assembled only from recorded records | integration | pass |
| Manual references through the CLI (`--references`) | run | pass |
| USPTO PatentsView adapter, records marked verified | unit, 3 tests against a loopback mock | pass |
| Opt-in records verified references; opt-out queries nothing | integration | pass |
| Denied opt-in sends zero requests | unit | pass |
| Research end to end in the window: opt-out sends nothing, opt-in sends exactly one query, the reference is shown | installed + mock, 8/8 twice | pass — `docs/evidence/research/` |
| A live PatentsView query | — | **blocked — key** (never claimed as passing) |

## Phase D — setup and lifecycle

| Item | Evidence | Result |
| --- | --- | --- |
| Projects, intake, optional context fields | installed | pass |
| Local endpoint and model configuration | installed | pass |
| Model picker from the runtime's own list, with readiness stated as count / empty / unreachable | installed + mock, 4/4 twice | pass — `docs/evidence/models/` |
| Usage records; unknown is not zero | unit, run | pass |
| First-run profile prompt; the choice persists; Lean refuses Local, Full accepts it | installed, 6/6 twice | pass — `docs/evidence/first-run/` |
| Single instance refused with exit code 3 and a stated reason | installed, 5/5 twice | pass — `docs/evidence/lifecycle/` |
| Quit closes when nothing is pending; the busy-quit guard | installed for the first half; session tests for the guard | pass — the window half is not claimed |

## Phase E — migration

| Item | Evidence | Result |
| --- | --- | --- |
| Copy-first importer: fixture, preview counts, provenance warnings, preserved archive, re-runnable, source untouched, missing source refused | run, 16/16 twice | pass — `docs/evidence/migration/` |
| Credentials redacted at import (column recorded, value never carried); re-entered per session | unit + run | pass — the proof scans the activated database bytes for the fixture's key material |
| Read API contracts verified over loopback; CLI matrix and DSH compatibility documented | run, 13/13 twice | pass — `docs/API-CONTRACTS.md`, `docs/evidence/api/` |
| HTTP write endpoints | — | **not verified** (they change state; nothing exercised them) |
| A real PatentForge database | — | **blocked — authorization and a copy** |

## Phase F — packaging

| Item | Evidence | Result |
| --- | --- | --- |
| One Windows installer from one revision, both profiles, SHA-256 recorded | run | pass — `docs/RELEASE-MANIFEST.md`, revision `e59f4be`, `d1a81645…`, 12115502 bytes, unsigned |
| The release workflow builds the requested tag, versions the installer from it, requires versioned notes, and refuses to overwrite a published release without `replace: true` (never rewriting notes) | review + YAML parse | pass — F-005, F-006 |
| Windows install, upgrade, rollback, uninstall, data retained | run, 11/11 twice | pass — `docs/evidence/upgrade/` |
| Signature status recorded truthfully | review + `Get-AuthenticodeSignature` | pass — v0.1.0 signed; candidate unsigned |
| Licence metadata agrees with the committed licence | run, `cargo metadata` reports MIT for every crate (`license` field, not only a metadata key) | pass — F-045 |
| macOS artifact of this revision | — | **blocked — authorization** — an unsigned, unnotarised `.app` of `bb68c0f` exists locally (`e852fc22…`); publishing it is Scott's decision |
| Linux artifact of this revision | run in CI (`ubuntu-24.04`): release build, tarball, SHA-256 | pass — `patent-forge-lite-v0.2.1-linux-x64.tar.gz`, `7e9046ed…`, uploaded as a build artifact |
| The shipped Linux binary starts on a real desktop display | run in CI under Xvfb with `mesa-vulkan-drivers` | pass — process alive after 10 s and its profile created; a human has still not used it interactively |
| One provider registry defines every provider, its budget and its lists; a stored kind this build does not know opens as Replay instead of failing | unit (`registry`, `provider_kind_compat`) | pass — 13 tests, including list/registry agreement, slug round-trips and a captured 0.2.1 payload |
| Per-surface provider assignment (three ranks), per-install provider overrides and operator connections persist; a connection row stores no credential; an older database opens unassigned | integration (`assignments`, 10 tests) | pass — CRUD, rank refusal, cascade, reopen, and a payload grep for credential shapes |
| Effort reaches the provider: low/medium/high become an Anthropic thinking budget, and an unknown word is refused before any request | integration (`anthropic_effort`, 4 tests) | pass — 1024/4096/8192 in the body; no field when unset; unknown effort refused |
| The operator's own Claude CLI runs as a provider: spawn, parse the result envelope, kill the tree on cancel or timeout, never store a credential | unit (`claude_cli`, 10 tests) + one live call on this machine | pass — captured envelope with its warning line, error/empty/timeout/exit-code cases, redaction, effort in argv; live call returned "ok" with usage 10/81 |
| The operator's own Codex CLI runs as a provider: JSONL events parsed to the last agent message, usage from `turn.completed`, a failed turn's nested reason unwrapped, prompt on stdin | unit (`codex_cli`, 8 tests) + one live call on this machine | pass — captured stream parsed; live `gpt-6.1-sol` call returned "ok" with usage 24845/5; the account's default model fails with model_not_found and that is a test case |
| The operator's own Gemini CLI runs as a provider: one JSON object parsed, the auth refusal named, prompt on stdin | unit (`gemini_cli`, 6 tests) | pass — success, auth refusal, empty answer, non-JSON, fake program, missing program |
| Gemini live call | — | **blocked — authorization** — the installed CLI (0.71.1) reports "Please set an Auth method"; Google sign-in or `GEMINI_API_KEY` is the operator's to give |
| Sign-in state is asked of the installed CLIs, with no model call: `claude auth status` JSON and `codex login status` (which prints its sentence to stderr) | live probe on this machine (`cli_login`, ignored test run once) | pass — both report signed in |
| Login attempts record the authorize URL, one-time code and expiry, never a credential; a finished or expired attempt is not open | unit (`cli_login` parsers, 5 tests; `assignments` login rows, 3 tests) | pass — URL read from the sentence, banner version rejected, ANSI stripped, expiry honoured |
| A sign-in can be started, polled and cancelled: URL and code surfaced while it waits, outcome from the program's own exit, one attempt per provider, cancel kills the tree | unit (`login_manager`, 5 tests) | pass — URL then finish, redacted failure, cancel, one-at-a-time, late URL |
| A real re-authentication completed end to end | — | **blocked — authorization** — both CLIs are already signed in here; re-auth opens a browser, which is the operator's to complete |
| Local servers are discovered with what each has loaded: LM Studio `state`, Ollama `/api/ps`, and a plain OpenAI server honestly reports unknown | unit (`local_discovery`, 6 tests) + live probe on this machine | pass — LM Studio 15 models, Ollama 36, none loaded; a dead address is absent, not an error |
| "Use whatever is loaded" is the lowest loaded non-embedding model; nothing loaded picks nothing | unit (`local_discovery`) | pass — deterministic choice, embeddings excluded, idle models never chosen |
| Automatic tries rungs in order and advances only on quota, unavailable, lapsed sign-in, timeout, no output or unreadable answer; a refusal or unrecognised failure stops and is shown | unit (`ladder`, 6 tests) | pass — the quota-then-success path records `skipped: claude / quota`, a refusal is not retried elsewhere, an unbuildable rung is recorded |
| The ladder can answer as a single provider, so a failing stage moves to the next rung without the service knowing about ladders | unit (`ladder_provider_tests`, 3 tests) | pass — a quota failure returns the second rung's text and records it answered; a refusal does not walk the ladder; an empty ladder is an error; and the session now builds this ladder from a surface's ranked assignments |
| A run records what it asked for, what answered, the effort and which rungs were skipped and why | unit (`ladder`, receipt fields) | pass — `requested`, `actual`, `effort`, `skipped[{rung, reason}]` |
| One catalogue joins the registry, the discovered servers and the CLI sign-in states; every provider appears once with a reason when it cannot be used | unit (`catalog`, 5 tests) + live `pfl-cli models` on this machine | pass — claude/codex signed in, gemini installed not signed in, LM Studio 15 models, Ollama 36, llama.cpp silent |
| The Models page renders every provider with its availability and reason, and refreshes on demand | installed, driven on this machine; screenshot `docs/evidence/models/20261126T1130Z/` | pass — 8 rows: Claude/Codex available, Gemini not signed in, Anthropic/OpenAI not configured, LM Studio 15, Ollama 36, llama.cpp silent |
| A model assignment can be read and written per project and surface: rank 0 is the choice, 1–2 the fallbacks, and an empty provider clears the surface | unit (`assignments`, storage) + session request/reply | pass — the reply carries the record, so the page redraws from what was stored |
| A provider can be chosen from the Models page: one control writes rank 0 for every workflow, the page redraws from the record, and unavailable providers offer nothing | installed, driven on this machine; screenshot `docs/evidence/models/20261126T1130Z/` | pass — choosing Claude wrote four `workflow-*` rows (verified in the database) and the button read "In use" |
| Automatic is a choice that clears the record rather than pinning something | installed, driven on this machine | pass — Automatic removed all four workflow rows; the database held zero afterwards |
| A run uses the assigned loopback provider, and an off-machine assignment falls back to the configured mode until consent covers it | unit (`assigned_tests`) + start path | pass — the mapping accepts the three local servers and refuses claude, codex, gemini, anthropic and openai by name |
| One consent rule covers every provider kind: off-machine refuses real-invention data, hosted APIs need paid approval, subscriptions do not, local and replay keep their own rules | unit (`data_rule_tests`, 4 tests) | pass — the matrix is tested by kind, including the five off-machine kinds |
| An assignment naming Claude, Codex or Gemini builds that program for the run; hosted key kinds still fall back to the configured mode | unit (`assigned_tests`) + consent matrix | pass — mapping accepts the three subscriptions and the three local servers; anthropic/openai deliberately not yet wired |
| A provider can be named as the fallback (rank 1) from the Models page, and the page reads it back | installed, driven on this machine | pass — choosing a fallback wrote four rank-1 rows and the button read "Is fallback" |
| A provider can be assigned to one stage, not only a workflow, and read back | run (`pfl-cli models --set-surface stage-prior-art --provider claude --rank 0`) | pass — the database held stage-prior-art rank 0 claude and rank 1 codex |
| A reader can learn what the provider layer does, what it refuses, and what is missing, from `docs/PROVIDERS.md` | review | pass — the kinds, the consent rule, the ladder taxonomy, and the four known gaps are stated there |
| An installed-but-stopped Ollama is started; a server that answers is never spawned; a spawn that never serves is a failure | unit (`ollama_supervision`, 5 tests) + wired into discovery | pass — every branch by fake probe/spawn; on this machine Ollama was already running and was left alone |
| The Automatic order this machine would try is shown, not implied | run (`pfl-cli models` now reports `automatic`) | pass — this machine reports `automatic: ["claude"]`, which is the registry ladder filtered to what is reachable |
| A run can carry the provider plan behind its choice, and an older run without one still parses and permits only its own provider | unit (`provider_plan_tests`, 3 tests) | pass — plan membership authorizes, a non-member is refused, an older payload parses with an empty plan |
| A checkpoint records the provider that answered it, so a receipt cannot claim another rung; an older checkpoint without the field still reads | `Checkpoint.provider` (serde default) + legacy migration sets `None` | pass — the answering provider is recorded on every new checkpoint and legacy ones stay `None` |
| A service's runs carry the provider plan the service was given, and an empty plan keeps today's behaviour of permitting exactly the named provider | unit (workspace regression, `permits`) | pass — all three run constructions use the service's plan; empty is the default |
| The plan can be declared on a service before it is shared, so a caller outside the workflow crate can set it | `set_provider_plan` (public API) + workspace regression | pass — the setter is exercised by the full suite through the default empty plan |
| A run's stages can be served by different providers: the session precomputes a stage→provider map, gives the service a resolver, and declares the plan before sharing | run-level test (`a_resolver_serves_every_stage_and_each_request_names_who_answered`) | pass — a resolved run names its answering provider on every request; a second test proves two providers answering two different stages, each recorded, with both in the plan |
| An `anthropic` assignment builds the hosted provider from the session key, and falls back to the configured mode when no key is set | workspace tests + native clippy at this revision | pass — the key comes from the session, never the database; no key is configured here, so no hosted run was observed |
| A saved custom connection can serve a run: an assignment naming `conn-<id>` uses exactly that endpoint and model | workspace tests + native clippy at this revision | pass — the connection is looked up by id, its own model is the fallback; a keyed connection waits for the connection UI |
| A provider can be named for the next run only; that run spends it and the project's assignments are untouched | workspace tests + native clippy at this revision | pass — the control is on the Models page and the start path spends the override with a plan of exactly its own snapshot |
| The Models page lists the project's assignments (surface, rank, provider) as a record, not only as an "In use" marker | workspace tests + native clippy at this revision | pass — the list renders from the stored rows after a refresh |
| The Models page lists saved connections (name, id, endpoint), so an endpoint used as `conn-<id>` is visible | workspace tests + native clippy at this revision | pass — the panel renders from the stored rows after a refresh |
| A connection can be added: the record is saved, listed, addressable as `conn-<id>`, carries no credential, and nothing is contacted by saving one | session tests (`tests/connections.rs`, 2 tests) | pass — saved and listed; a nameless or URL-less connection is refused and nothing is stored |
| A provider can be assigned to the final report stage alone, from the page | workspace tests + native clippy at this revision | pass — writes `stage-final-report` rank 0 for the open project and marks the control when it is set |
| A run uses a stage-level assignment | — | **not verified** — the resolution path and the two-provider run are proven (see the row above), but no run has been observed reading a stage-level assignment from the store: no local model is loaded here and a subscription run would spend the operator's quota |
| Any OpenAI-compatible server works as a provider — LM Studio, llama.cpp, vLLM, Ollama's own `/v1` — detected from the endpoint instead of assuming Ollama | live probe + unit (`openai_frames`, 11 tests) | pass — streamed answers from both servers here; frames, key header, error shapes, catalogue and cancellation pinned against a loopback stand-in |
| Whole test suite on Apple silicon hardware — measured **before** the host upgrade, macOS 26.5.2 (2026-11-24), re-run on 27.2.1 | run over SSH on a real Mac mini; transcript `docs/evidence/macos/20261125T023619Z/` | pass — 246 passed, 0 failed, exit 0 on both OS versions |
| macOS bundle script: version templated from `PFL_VERSION`, `docs/evidence` excluded — macOS 27.2.1 (2026-11-25) | run on the same Mac: `bundle.sh`, recorded in `docs/evidence/macos/20261125T023619Z/` | pass — plist `0.2.0`; `Resources/docs` present, `docs/evidence` absent |
| The non-GUI product on macOS arm64 — macOS 27.2.1 (2026-11-25) | run on the Mac mini: pipeline, revisions, package, inspect, validate-claims; transcript `docs/evidence/macos/20261125T023619Z/` | pass — 4 workflows/19 stages; 8 package files, 7 manifest hashes matched; CSV starts with the UTF-8 BOM |
| macOS native build and app bundle on real hardware — macOS 27.2.1, Xcode 27.2 (2026-11-25) | run: transcript `docs/evidence/macos/20261125T023619Z/` | pass — Mach-O arm64 binary; 21 MB `.app`, version `0.2.0`, `docs/evidence` absent, ad-hoc linker-signed (not Developer ID, not notarised) |
| The bundled macOS app launches, creates its profile and quits cleanly — macOS 27.2.1 (2026-11-25) | installed: `open -n`; transcript `docs/evidence/macos/20261125T023619Z/` | pass — schema v1 with `run_sequences`, 0 projects/runs, quit via AppleScript |
| The bundled macOS app opens an existing workspace — macOS 27.2.1 (2026-11-25) | installed: seeded 1 project / 4 runs, launched; transcript `docs/evidence/macos/20261125T023619Z/` | pass — process alive, counts unchanged; profile removed afterwards |
| Interactive UI acceptance on macOS (driving the window) | — | **blocked — harness** — the UI harness is PowerShell/UIAutomation and does not run on macOS; AppleScript automation needs an Accessibility grant on that machine |
| Documentation agreement at this revision | review | pass — the manual, README, STATUS, site, release notes and packaging describe this build; F-040 to F-048 are closed |
| This matrix checks itself, in CI (13 checks): counts match rows, evidence paths exist, no stale claims, no single-stage promise, no empty table, no note claiming work is ongoing, and the installer is not behind shipped code | run (`scripts/acceptance-check.ps1`) + CI step | pass — the freshness rule was proven to fail by pointing it at an older baseline (29 shipped files reported), and the others were proven on injected bad counts, paths and claims |

## Open defects

This section previously read **"None. F-001 to F-077 are closed."** That was wrong at this
revision. An independent review of HEAD reports the following, with locations, and none of them is
closed by this matrix:

- `Resume` never calls `ensure_idle` — the guard is used at `session.rs:434, 638, 770, 849, 907, 957,
  982`, and the `Resume` arm at `:1082` has none.
- Copy and Cut discard their handler (`crates/pfl-desktop/src/native/input.rs:461-467`).
- `report_runs` paints body text in the paper colour (`native/mod.rs:3050+`).
- `StartSelected` does not check the mode.
- `history` uses a `LEFT JOIN` on `run_sequences` (`crates/pfl-storage/src/lib.rs:894, 1137`).
- `serve` requires `--addr` (`crates/pfl-cli/src/main.rs:963`).
- The comment on `Resume` ends mid-sentence.

They are not entered as matrix rows because they have not been triaged here; the `open defect` count
therefore still reads 0, and that number is not evidence that the list is empty. Until each is fixed
or refuted at its cited line, "code complete" is not a claim this repository can make.

The navigation defect is closed: the rail is a tab list, each item is a tab, and the UI
harness measures it as `TabItem` with `SelectionItemPattern` (38/38).
## Decisions that remain Scott's

1. **Version — decided: 0.2.1.** The crates, the installer's `AppVersion`, the manifest and
   `docs/RELEASE-NOTES-0.2.1.md` all say 0.2.1, so the artifact name is no longer an override.
2. **Signing.** The Windows candidate is unsigned: the signing step has not been run against it.
   The macOS app cannot be signed at all — there is no Apple certificate — so macOS ships unsigned
   by policy, not by omission.
3. **Publication.** Nothing is tagged or published. Publishing and the release notes are a
   separate decision.
4. **PatentsView key.** One live query would close the last Phase C row.
5. **Migration authorization and a real database copy.** That closes Phase E2.
6. **Linux.** The artifact and a headless launch are now built and run in CI; a human has still
   not used it on a desktop.
7. **The `impl/*` branches.** Keep or prune; deciding is not reversible without their SHAs.
## What this document does not claim

- That the candidate is installed acceptance on macOS or Linux.
- That any live provider has been contacted.
- That a real legacy database has been migrated.
- That the HTTP write surface works.
- That every documentation page matches the build while F-040 to F-048 remain open.
```

## 17. Update: the seven defects are closed (work after `594fdb7`)

Everything in §3 that describes open defects is superseded by this section. Between `c6e6546` and
`b6c31f3` each of the seven defects listed in `docs/ACCEPTANCE.md` was fixed and closed by a test that
**was run with its fix removed and failed**; the command and the output are in each closure commit's
body, and `docs/ACCEPTANCE.md` carries one evidence line per defect naming its test and that commit.

| Defect | Commit | Test |
| --- | --- | --- |
| D1 Resume bypasses the single-run guard | `c6e6546` | `resuming_while_another_run_is_in_flight_is_refused_and_sends_nothing`, `resuming_is_refused_while_any_run_is_in_flight` |
| D2 Copy and Cut are dead | `98ecef5` | `editor::clipboard_tests::a_masked_field_never_reaches_the_clipboard` |
| D3 Report label in the page colour | `e306416` | `native::report_colour_tests::report_labels_are_not_drawn_in_the_page_colour` |
| D4 Compliance starts with no mode | `e0f11be` | `state::mode_gate_tests::...`, `compliance_start_without_a_mode_is_refused_and_creates_no_run` |
| D5 `inspect` cannot open a v0.1.0 database | `c03502e` | `a_v0_1_0_database_reads_read_only_and_is_not_modified`, `the_read_write_path_migrates_the_same_file_and_agrees` |
| D6 `serve` requires `--addr` | `46771a2` | `tests::serve_without_addr_...`, `serve_takes_the_loopback_default_when_no_addr_is_passed` |
| D7 The `Resume` comment | `c6e6546` | a comment, not behaviour; no test applies |

Also done in that window: `start_locked` now refuses a second run anywhere (`5f013c0`), so concurrent
projects are not supported by this build; and the secondary list from the review is fully
dispositioned in `docs/ACCEPTANCE.md` — ten items fixed with tests, two recorded with their reasons
(soft-wrap navigation, and Tab/Enter plus `aria_value`, both of which need the accessibility layer).

The release items that were open at `594fdb7` — T8 (Gemini sign-in), T17–T19 (live paid verification,
PatentsView, three-platform acceptance) and T20 (signing) — are **unchanged and still open**: they need
the operator's credentials, machines and certificate, not code.
