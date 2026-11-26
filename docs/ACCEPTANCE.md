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
| Revision | `da6bcc5` |
| Candidate version | 0.2.1 — declared in `Cargo.toml` and named by the installer, so there is one version, not an override |
| Published release | v0.1.0 only; this revision is **unpublished and unsigned** |

## Summary

| Status | Count |
| --- | --- |
| pass | 149 |
| not verified | 1 |
| blocked — hardware | 0 |
| blocked — key | 1 |
| blocked — authorization | 4 |
| blocked — harness | 3 |
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
| A run records what it asked for, what answered, the effort and which rungs were skipped and why | unit (`ladder`, receipt fields) | pass — `requested`, `actual`, `effort`, `skipped[{rung, reason}]` |
| One catalogue joins the registry, the discovered servers and the CLI sign-in states; every provider appears once with a reason when it cannot be used | unit (`catalog`, 5 tests) + live `pfl-cli models` on this machine | pass — claude/codex signed in, gemini installed not signed in, LM Studio 15 models, Ollama 36, llama.cpp silent |
| The Models page renders every provider with its availability and reason, and refreshes on demand | installed, driven on this machine; screenshot `docs/evidence/models/20261126T1130Z/` | pass — 8 rows: Claude/Codex available, Gemini not signed in, Anthropic/OpenAI not configured, LM Studio 15, Ollama 36, llama.cpp silent |
| A model assignment can be read and written per project and surface: rank 0 is the choice, 1–2 the fallbacks, and an empty provider clears the surface | unit (`assignments`, storage) + session request/reply | pass — the reply carries the record, so the page redraws from what was stored |
| A provider can be chosen from the Models page: one control writes rank 0 for every workflow, the page redraws from the record, and unavailable providers offer nothing | installed, driven on this machine; screenshot `docs/evidence/models/20261126T1130Z/` | pass — choosing Claude wrote four `workflow-*` rows (verified in the database) and the button read "In use" |
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

None. F-001 to F-077 are closed; each is cited in `docs/BUILD-EVIDENCE.md`.
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