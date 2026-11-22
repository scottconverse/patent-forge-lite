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

After all four workflows were implemented on top of the storage, workflow and
desktop lanes, and the CLI harness and compatibility API were written, the
following was measured on this machine with the pinned `1.98.1` toolchain,
`--locked --offline`, and `jobs = 2`:

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed (all crates). |
| `cargo test --workspace --locked --offline` | Passed: **96 tests** — domain 8, storage 24, workflow 8 unit + 9 + 1 + 1 + 7 + 5, CLI 6, desktop 24, API 3. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed. |
| `cargo clippy -p pfl-desktop --features native --all-targets ...` | Passed. |
| `cargo check -p pfl-desktop --features native --locked --offline` | Passed. Real compile of the native app source. |
| `cargo build -p pfl-desktop --features native --locked --offline` | Passed and linked `target/debug/pfl-desktop.exe` (32.76 MiB). The binary was not launched. |
| Workflows end to end | `pfl-cli pipeline` completes all four workflows in dependency order: 19 stages, 19 recorded provider calls, then reopens the profile with zero further generation. |
| Word export | A `.docx` container is opened back up in tests and its `word/document.xml` contains the saved report text. |
| Provider wire formats | Cloud SSE and Ollama NDJSON are exercised against local mock servers; unauthorized cloud runs make no request at all. |
| Research seam | Supplied references are snapshotted into the run input and reach the prior-art prompt; unauthorized runs query nothing. |
| Setup profiles | Lean refuses a local runtime; Full accepts it. |
| HTTP compatibility | Bearer, both legacy 401 messages, tokenless mode, long-lived query token, and single-use 5-minute download tokens verified. `pfl-cli serve` was exercised as a real process over a seeded profile. |
| CLI hard-kill recovery | Passed in a genuinely fresh process at stage 1 and stage 4, with no repeated checkpoints. |
| CLI shared-profile lock | Passed: a second owner is refused while the first holds the profile; the OS lock releases on hard kill. |
| Prompt identity | All seven feasibility prompts, plus the claims, compliance and application sets, are byte-identical (SHA-256) to the frozen legacy sources. |
| **Live on-machine inference** | **Passed**, 2026-11-21. `pfl-cli live` against the already-running local Ollama at `http://127.0.0.1:11434` with `phi4-mini:latest`, synthetic fixture input only. All four workflows completed in dependency order: **19 stages, 19 real provider calls, 715.5 s**. Saved reports: feasibility 12 212 B, claims 34 678 B, compliance 66 904 B, application 56 040 B. The profile reopened with four versions and zero further generation. No model was downloaded and no independently owned process was started or stopped. An earlier feasibility-only live run (six stages, 189.5 s) is also recorded. |

### What the live runs showed

The model produced coherent, on-topic material — real numbered claims with
broad/medium/narrow independents and dependents, and a full application with
abstract, background, summary, drawings listing, detailed description and an IDS
table. It also **invented prior-art reference numbers** (for example "12345678 /
Temperature Sensor in Coffee Cup") because no research source was configured. That
is precisely the failure mode the embedded disclaimers warn about, and it is why
the output is labelled research material rather than verified prior art. Real
research remains a separate integration.

Still not established by this evidence:

- No native window was launched; keyboard, IME, GPU rendering, dialogs and
  installed behavior remain unverified interactive acceptance. A linked binary is
  a build result, not usability evidence.
- No live **Cloud** inference ran; the adapter is verified against a mock only and
  remains fail-closed without explicit paid authorization.
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

## Windows installer, 2026-11-21

First package ever built for this product. **Unsigned** — no signing policy has
been chosen yet, so SmartScreen will warn on first run.

| Fact | Value |
| --- | --- |
| Build command | `cargo build -p pfl-desktop --features native --release --locked` (7m27s) then `iscc packaging\windows\patent-forge-lite.iss` (6.2s) |
| Inno Setup | 6, at `%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe` |
| Artifact | `dist\patent-forge-lite-0.1.0-x64.exe`, 7.15 MB |
| SHA-256 | `10A165F366EAF0FBE18C3C26A938A5C299F9A6960F822F5C6A9E5286EF17DC77` |
| Install scope | Per-user, no elevation: `%LOCALAPPDATA%\Programs\PatentForge Lite` |
| Installed payload | `pfl-desktop.exe` (19.6 MB), docs, all prompt files, Barlow fonts with OFL, brand icon |

Verified on this machine:

1. Silent install (`/VERYSILENT /DIR=...`) exited 0 and wrote the uninstall key
   under `HKCU`.
2. The **installed** binary launched, exposed its accessibility tree (15 nodes) and
   **passed all 26 UI acceptance checks** when the suite was pointed at the
   installed path (`scripts/ui-acceptance.ps1 -Exe ...`).
3. Start-menu shortcut created.
4. Silent uninstall exited 0, removed the install directory and shortcut, and
   **preserved** `%LOCALAPPDATA%\patent-forge-lite` (the user's project history).

Not done: code signing, an upgrade path across versions, macOS/Linux packages.

## GitHub Actions dispatch, 2026-11-21

The manual native workflow was dispatched once and **did not run**: all three jobs
were rejected before starting with

> The job was not started because recent account payments have failed or your
> spending limit needs to be increased.

That is a GitHub billing condition on the account, not a workflow or code failure.
`workflow_dispatch` triggered (run 44949798709), so the definition itself is
reachable; no runner ever executed a compile. macOS and Linux therefore remain
**never compiled**, and the platform row stays open until billing is resolved.
## Cross-platform CI, 2026-11-21

The repository was made public (unblocking GitHub's billing restriction on private
Actions minutes) and the manual native workflow was dispatched:
run [`44950601970`](https://github.com/scottconverse/patent-forge-lite/actions/runs/44950601970).

| Runner | Target | Result | Time |
| --- | --- | --- | --- |
| `windows-latest` | `x86_64-pc-windows-msvc` | passed | 8m51s |
| `macos-15` | `aarch64-apple-darwin` | passed | 6m02s |
| `ubuntu-24.04` | `x86_64-unknown-linux-gnu` | passed | 4m59s |

Each job ran `cargo fmt --all -- --check`, `cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`cargo build -p pfl-desktop --features native --locked` for its own target. All
three passed on the first attempt, so the code compiles and its tests pass on
macOS arm64 and Linux x86_64 — previously never compiled at all.

What this still does not establish: no installed package, no launched window and no
interactive behaviour on macOS or Linux. Those remain separate acceptance steps.

### History scrub and a residual exposure

The public repository history was rewritten before publishing to remove the
screenshots committed by mistake on 2026-11-21 (including a full-desktop capture
showing unrelated windows and personal folder names). `filter-branch` plus a full
`gc --prune=now` removed them from every ref; a fresh clone of the public remote
contains only `assets/brand/app-256.png`.

Residual and unresolved: GitHub still serves the pre-rewrite commit
`423c2ead695099ba96807a5ec697a960cdde4cd4` by its ID, and that tree still contains
`session-screen.png` and the early `shots/*.png`. Unreachable objects are not
immediately garbage-collected and no API is exposed for forcing it. The SHA was
never published or linked, so the practical exposure is low, but it is not zero.
Options: ask GitHub Support to purge unreachable objects, or delete and recreate
the repository (same name and URL, clean object store, empty Actions history).
## Release v0.1.0, 2026-11-21

Tags and publishes through `.github/workflows/release.yml` (run 44953930049).

| Asset | Size | State |
| --- | --- | --- |
| `patent-forge-lite-v0.1.0-linux-x64.tar.gz` | 16.44 MB | Published, unsigned; sha256 sidecar verified against the download |
| `patent-forge-lite-v0.1.0-macos-arm64.zip` | 7.74 MB | Published, unsigned; contains a real Mach-O arm64 `pfl-desktop` (magic `cf fa ed fe`) |
| Windows installer | — | **Not published**: the repository has no Azure secrets yet, and the pipeline refuses to publish an unsigned Windows installer |

Both artifacts were downloaded again from the public release and inspected: the Linux tarball
carries the binary, docs, all prompt files, Barlow fonts, the `.desktop` entry and notices; the
macOS zip is a well-formed `.app` bundle. CI: fmt, tests, clippy and the native build pass on all
three runners; the macOS and Linux jobs each produced their package in CI.

Landing page: https://scottconverse.github.io/patent-forge-lite/ (GitHub Pages, `main:/docs`).

Remaining gate: signing the Windows installer requires `AZURE_CLIENT_ID`, `AZURE_TENANT_ID` and
`AZURE_CLIENT_SECRET` in this repository, mirroring the CivicCast pipeline (account
`scottconverse-signing`, profile `ScottConversePublic`, endpoint `wcus.codesigning.azure.net`).
With those present the Windows job builds, signs, verifies and the publish step attaches the
installer to the existing v0.1.0 release.
## Windows signing, 2026-11-21 (Azure Trusted Signing)

Signed through `scottconverse/civiccast` -> `.github/workflows/sign-native-installer.yml`,
per the one full signing procedure. Two runs, one per file, in the required order (app binary
first, then a rebuilt installer that contains it).

| File | Unsigned SHA-256 | Signed SHA-256 | Run |
| --- | --- | --- | --- |
| `pfl-desktop.exe` (application) | `9738f5d7…ee423` | `7abf0c77…b8a71` | [44965745433](https://github.com/scottconverse/civiccast/actions/runs/44965745433) |
| `patent-forge-lite-0.1.0-x64.exe` (installer) | `3cdcdcfa…991f2` | `84da4443…a194edbb` | [44966583618](https://github.com/scottconverse/civiccast/actions/runs/44966583618) |

Both: `Status: Valid`, signer `CN=Scott Converse, O=Scott Converse, L=Longmont, S=co, C=US`,
issuer `Microsoft ID Verified CS EOC CA 03`, RFC3161 timestamp from `Microsoft Public RSA Time
Stamping Authority`, and each file's SHA-256 equals `signed_sha256` in its receipt.

The installer was rebuilt only after `target\release\pfl-desktop.exe` was replaced with the signed
binary, so it embeds the signed executable. Proven by installing it: the installed
`pfl-desktop.exe` hashes to `7abf0c77…b8a71` and its signature status is `Valid`. Uninstall
removed the application and preserved the profile.

Draft intake releases created in civiccast, left as drafts and never published:

- `patentforgelite-intake-9738f5d7-20261121-171547` (app)
- `patentforgelite-intake-3cdcdcfa-20261121-172604` (installer)

Published asset: `patent-forge-lite-0.1.0-x64.exe` (7.86 MB) plus `.sha256` on the v0.1.0
release, re-downloaded and re-verified after upload.

One process note, recorded because it happened: the first run used an earlier helper script that
failed to locate the run it had dispatched and stopped. It had already dispatched a successful
run; that run was recovered from the run list and verified (never re-dispatched). The full
procedure's script replaced it for the installer and printed `SIGNED OK`.
## Increment A (candidate, 2026-11-21) — not published

Scope: make the product truthful and dependable. No release was tagged, no artifact was replaced.

### Code changes

- **Recorded data declaration.** `Authorization` gains `real_data` (serde-defaulted, so pre-change
  snapshots still deserialize). Every run now requires one of two recorded declarations; replay
  refuses a real-invention declaration because recorded output is synthetic by construction.
  The desktop offers both declarations and records the chosen one in the immutable run input.
- **Loopback enforcement.** `OllamaProvider::new` rejects any non-loopback endpoint, so a remote
  runtime cannot be presented as on-machine inference. `is_on_machine()` is now a pure helper,
  not a label applied after the fact.
- **Truncated outcomes.** Both providers must see a terminal frame (`done` / `message_stop`).
  A stream that ends early returns a dedicated `Incomplete` error, which the service records as
  `RunOutcome::Truncated` — never `Completed`. Truncated runs are resumable.
- **Compliance label.** The stage prompt no longer calls the feasibility analysis a
  "Specification", and the Analysis page states that the review does not examine a drafted
  specification.

### Evidence

| Check | Result |
| --- | --- |
| `cargo test --workspace` | **111 passed** (was 104; +5 consent, +2 provider) |
| `cargo fmt --check`, `clippy -D warnings` | clean |
| New: consent rules (`tests/consent.rs`) | no declaration refused; replay+real refused; synthetic accepted and recorded |
| New: old-snapshot compatibility | an `Authorization` JSON without `real_data` parses, field defaults false |
| New: fragmented chunks | a UTF-8 character split across chunks reassembles (`café 🧪`) |
| New: missing terminal frame | `Incomplete`, not a completed result; cloud fixture needed its `message_stop` added — the rule caught a real gap in the fixture |
| Loopback via CLI | `pfl-cli live --endpoint http://192.0.2.10:11434` → refused with the policy message; a loopback endpoint passes policy and fails later on connection |
| Installed-build UI acceptance | candidate installed to `%LOCALAPPDATA%\Programs\PatentForge Lite`, **28 checks passed** via `ui-acceptance.ps1 -Exe <installed>` |
| Candidate installer | `dist\patent-forge-lite-0.1.0-x64.exe`, 7.47 MB, SHA-256 `b08c68dabbde0e7f2886583b590947aa123b438f0ba8d28d19134e342c5c840d`, **unsigned** |

Not done in this increment: reference lineage, application-revision review, rendered HTML, IDS
assembly (Increment B); Cloud UI; macOS/Linux installed acceptance (those platforms are now
described as artifacts only).
## Increment A — audit fixes (candidate 2, 2026-11-21)

The first Increment A candidate was audited and **not signed or published**. The audit found:

1. **Blocker — real-invention analysis still failed.** The desktop and service accepted the
   acknowledgement, but `OllamaProvider::generate` and `CloudProvider::generate` still required
   `synthetic_data`, so every real-declared run was refused at the adapter. The desktop Resume
   button had the same requirement. The positive "real-data" test named above actually exercised
   synthetic replay.
2. **Critical — output limits looked like success.** Both adapters checked that a terminal frame
   arrived but ignored *why* generation stopped, so a `length` / `max_tokens` stop could be saved
   as a completed stage.
3. **Major — the UI harness was destructive.** It stopped every `pfl-desktop` process and replaced
   the operator's real profile database.

Fixes in this candidate:

- Live adapters require a *recorded data declaration* (synthetic or real) instead of requiring
  synthetic; replay still refuses real data. The desktop Resume guard accepts either declaration.
- Completion now requires a natural stop: `length`, `max_tokens` and `max_output_tokens` produce
  `Error::Incomplete` → `RunOutcome::Truncated`, never `Completed`.
- `scripts/ui-acceptance.ps1` seeds and runs against a throwaway `LOCALAPPDATA`, refuses to run if
  another instance is live instead of killing it, stops only the process it launched, and asserts
  the real profile's timestamp is unchanged.

Evidence (candidate `fb8db64a…510246`, unsigned, unpublished):

| Check | Result |
| --- | --- |
| `cargo test --workspace` | **115 passed**, exit 0, no failing suite |
| Real-invention Local run, loopback Ollama mock | `Completed`, `real_data = true` recorded, report contains the mock text |
| Truncated stream (no terminal frame) | `RunOutcome::Truncated`, no report |
| Output limit (`done_reason: "length"`) | `RunOutcome::Truncated`, no report |
| Installed-build UI acceptance, isolated profile | **31 passed**, including "the operator's real profile was not modified" |
| Real profile before/after | identical length and timestamp across harness runs |

Still held: no signing run, no publication, no Increment B.
## Increment A — recheck fixes (candidate 3, 2026-11-21)

The recheck accepted the Local fix but held the release for four reasons. All are now closed.

| Finding | Fix |
| --- | --- |
| Harness could recursively delete a caller-supplied `-ProfileRoot` | The harness creates its own directory, refuses an existing one outright, and only deletes a directory it created (`$ownsProfileRoot`). Verified: an existing directory is refused and survives. |
| Running-instance check logged a failure but continued | It now throws before seeding or launching, so it cannot drive another window. Verified: with a foreign instance live, the harness aborts and that process survives untouched. |
| Completion accepted unsupported stop reasons | Replaced the denylist with per-provider **allowlists**: Ollama completes only on `stop`; Cloud only on `end_turn` / `stop_sequence`. An absent reason, a limit, `tool_use`, `pause_turn` or any unknown value is `Incomplete` → `Truncated`. |
| Run could be both synthetic and real; Cloud widened to real data | Exactly one declaration is required (service and local adapter); Cloud requires synthetic data, so real-invention data is **Local-only**. `real_data + paid_provider` on Cloud is refused with an explicit message. |

The stricter stop rule immediately found three fixtures that had been passing without a stop reason
(two in `tests/providers.rs`, one in `tests/run_outcomes.rs`); all now carry `done_reason: "stop"` /
`stop_reason: "end_turn"`.

Evidence (candidate `f2c0ddeb…aa475`, unsigned, unpublished):

| Check | Result |
| --- | --- |
| `cargo test --workspace` | **117 passed**, exit 0 |
| fmt / clippy `-D warnings` | clean |
| Real-declared Local run (loopback mock) | `Completed`, `real_data` recorded |
| Missing terminal frame / `length` stop | `Truncated`, no report |
| Unsupported reasons (`tool_use`, `pause_turn`, unknown) | `Truncated` |
| Both declarations | refused: "exactly one data declaration" |
| Real data + Cloud | refused: "local-only" |
| Harness safety | existing `-ProfileRoot` refused and preserved; foreign instance aborts and survives |
| Installed acceptance, owned temp profile | **31 passed**, real profile untouched |

Still held: no signing, no publication, no Increment B.
## Item 1 closure: audit defects + installed Local proof (2026-11-21)

Closed the four remaining audit defects and produced the installed-app evidence the audit asked for.

### Fixes

- **Cloud stream integrity.** A malformed SSE frame is now a provider error instead of a frame to
  skip, and bytes left in the buffer at EOF (a stream cut mid-frame) are `Incomplete`. Previously
  either could silently drop text or a stop reason.
- **Version-correct Compliance resume.** Locked in by a regression test that interrupts a
  compliance attempt, records a *newer* claims set with distinguishable text, then resumes:
  every resumed prompt contains the claims version the run recorded and none contains the newer
  one. The stored upstream was already the source of truth; the test now prevents drift.
- **PID-bound UI automation.** `Get-Window` matches `ProcessId == the launched process`, so a
  second window with the same title can never be driven. The single-instance check aborts before
  launch.
- **Cleanup on error.** A `trap` plus an explicit `Clear-Harness` restores the environment, stops
  only the launched process, and removes only a directory the harness created — including when a
  check throws.

### Installed proof (real provider path, loopback mock)

`scripts/installed-local-proof.ps1` drives the **installed** application through UI Automation
against `scripts/mock-ollama.ps1` on loopback. No model is contacted, no paid provider is used.

| Check | Result |
| --- | --- |
| Real-invention declaration starts a Local run | **Completed, 6 of 6, version 5** |
| Stream cut after two stages | **Displayed as Truncated, 2 of 6, version 6** |
| Same run resumed | **Completed, 6 of 6, version 6** |
| Completed stages are not repeated | first run **6** requests, truncated **3**, resume **4** (exactly the 4 remaining) |

Evidence: `installed-local-proof.json` (exe hash, port, request counts, per-check results) plus
screenshots, written to a temporary evidence directory.

Installed candidate: `dist\patent-forge-lite-0.1.0-x64.exe`, SHA-256
`624f731cea661176ae9989bbba6234ee633af3d8b49314e0c151307b340c7f37`, unsigned, unpublished.
UI acceptance 31/31 and `cargo test --workspace` green on the same revision.
## Item 2 (engine): reference lineage, deterministic IDS, opt-in research (2026-11-21)

### Reference lineage

`PriorArtItem` is now a structured record — reference, title, source, publication number, filing
and grant dates, assignee, CPC, URL, relevance and a `verified` flag — every field serde-defaulted
so pre-change snapshots still deserialize.

The lineage defect is fixed at its root: `prior_art()` was a stub returning `Vec::new()`. It now
harvests the reference set recorded by the most recent completed feasibility run, so every
dependent workflow inherits exactly the set the operator supplied, and a resume cannot re-query or
drift.

`tests/reference_lineage.rs` proves a supplied two-record set reaches claims, compliance and
application unchanged, and that the generated Information Disclosure Statement lists exactly those
two records with title, dates, assignee, CPC, source and verification status.

### Deterministic IDS

The IDS is assembled only from recorded records (`report::ids_table`), never from model prose. The
rendered table now carries the structured columns, and an empty set still says so explicitly.

### Opt-in live research

`HttpResearchSource` implements one documented contract (`GET {endpoint}?q=&limit=`, optional
bearer key, JSON array) so a provider adapter or local proxy can sit behind it. It is opt-in: only
consulted when the run authorizes research and an endpoint is configured.

`tests/research_http.rs` verifies, against a loopback mock: retrieved records are structured and
marked verified; an empty array is a successful search that found nothing; a malformed body and an
HTTP 503 are errors; an invalid endpoint is refused before any request. No external network, no key,
no cost.

`Settings` gained `research_endpoint` (serde-defaulted, migration sets `None`).

Still to do for Item 2: desktop entry for manual references and the research endpoint, and a real
provider adapter chosen with Scott (API key/terms decision).
## Item 2 (entry points): CLI manual references and opt-in research (2026-11-21)

`pfl-cli pipeline|live` now accept `--references <json>` and `--research-endpoint <url>`.

- A reference file is parsed strictly: a malformed file is refused, never treated as an empty set
  (three unit tests).
- The set is passed to the feasibility run via `start_workflow_with_prior_art`, so every downstream
  workflow inherits it.
- When an endpoint is given, the service is built with `HttpResearchSource` and research is
  authorized for the run; settings record the endpoint.

Verified by a real pipeline run with two manual records: all four workflows completed (19 stages)
and the generated application report contains both records in the IDS with title, dates, assignee,
CPC, source and `Verified` status.

Still open for Item 2: a desktop entry point for references and the research endpoint, and choice of
a real provider (needs Scott's decision on provider and API access).
## Item 3 (engine start): versioned document revisions (2026-11-21)

Generation runs stay immutable. Edits are now separate, ordered revision records that anchor the
run they were edited from.

- `DocumentKind` (claims, application) and `DocumentRevision` (id, project, kind, version,
  `base_run_id`, text, note, created) in the domain, with `RevisionId`.
- SQLite table `document_revisions`, unique per `(project, kind, version)`; the service assigns the
  next version, so versioning is per document kind.
- `WorkflowService::{save_revision, revisions, revision}`. A revision may only anchor a run of the
  same project *and* the same document kind.
- Compliance can now run against **selected revisions**:
  `start_workflow_with_selection` records `claims_revision_id` / `specification_revision_id` in the
  immutable upstream snapshot and uses their text instead of the latest runs. The compliance prompt
  names the revisions under review.

Evidence: `tests/revisions.rs` (3 tests) proves version ordering, retrieval, that edits never mutate
the generation run, that application revisions are a separate series, that a mismatched anchor is
refused, and that a compliance run started with selections carries exactly the selected texts and
ids and completes.

129 workspace tests pass; fmt and clippy clean.

Still open for Item 3: desktop editing UI, targeted regeneration, revision comparison/history view,
revision-aware exports (package), and revision selection in the CLI/API.
## Item 3: deterministic claim-structure checks (2026-11-21)

`pfl_workflow::claims::validate` parses a claim set and checks structure offline: duplicate and
missing numbering, claims out of order, self-reference, reference to a nonexistent claim, forward
reference from a dependent claim, a dependent claim with no parent, and range/list references
(`claims 1-3`, `claims 2 and 4`). Issues carry a severity and the claim they belong to; the module
is deterministic and spends no provider call.

Independent claims that reference an earlier claim are accepted — that pattern is legitimate
("A method using the container of claim 1") and flagging it produced false positives, which the
tests caught.

Eight unit tests cover the clean case, duplicates and gaps, forward references, missing targets,
self-reference, ranges and lists, and empty/unnumbered text.

134 workspace tests pass; fmt and clippy clean.

Still open for Item 3: CLI/desktop surfacing of the check, revision comparison, targeted
regeneration, revision-aware export package.
## Item 3: revision-aware export package (2026-11-21)

`pfl_workflow::package::build_package` assembles a handoff package from **stored revisions**:
`claims.md` and `application.md` (each naming its revision id, version and base run), a
`references.csv` inventory with correct quoting, `compliance-findings.md` when a review was run, and
a `manifest.json` recording every file's SHA-256 and size plus the revision ids/versions and the
reference count.

Nothing is regenerated: the package describes exactly the revisions selected, and a slot holding the
wrong document kind, or an empty selection, is refused.

Five unit tests: file set and manifest hashes, CSV quoting of commas and quotes, findings inclusion
only when supplied, empty selection refused, wrong-kind slot refused.

141 workspace tests pass; fmt and clippy clean.