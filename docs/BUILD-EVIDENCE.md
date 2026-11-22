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
## Item 3: CLI surfaces for validation and packaging (2026-11-21)

`pfl-cli validate-claims --file <path>` prints the deterministic structural report as JSON and exits
non-zero when the set contains errors, so it can gate a pipeline.

`pfl-cli package --workspace <ws> --out <dir> [--claims <id>] [--application <id>]` writes the
handoff package from the stored revisions, using the reference set and compliance findings already
recorded for the project.

Verified by real runs: a well-formed set reports no issues and exits 0; a broken set reports three
structural errors and exits 1; `package` on a workspace with no revisions refuses with
"at least one selected revision" (correct — the pipeline creates runs, not edits). Creating a
revision still requires the library API; a CLI/desktop edit command is the next step, and until it
exists the successful package path is covered by `package::tests` rather than an end-to-end run.
## Item 3: the edit → validate → package chain is runnable (2026-11-21)

`pfl-cli save-revision --workspace <ws> --kind claims|application --from-run <id> --file <md>
[--note <text>]` stores an edited document as the next numbered revision of the project.

Verified as one real chain, not three commands:

1. `pipeline` created all four runs (19 stages).
2. The claims text was edited into a 3-claim set; `validate-claims` reported no issues.
3. `save-revision` returned revision `2c4cfc1f…`, version 1, anchored to claims run
   `cadbeac0…`.
4. `package --claims <id>` wrote `claims.md`, `references.csv`, `compliance-findings.md` and
   `manifest.json`; the manifest names that revision id and version 1.

The package's `compliance-findings.md` is the project's saved compliance report, and
`references` is 0 because this pipeline run supplied none — both correct for this input.

Gate with real exit codes: build 0, fmt 0, clippy -D warnings 0, workspace tests 0.
## Item 4 (start): loopback model discovery (2026-11-21)

`pfl_workflow::list_local_models(endpoint)` reads an on-machine Ollama's `GET /api/tags`
catalogue: loopback only, read-only, no inference, no runtime start or install. Names are sorted and
deduplicated. A remote endpoint is refused before any request.

Four tests against a loopback mock: sorted/deduplicated listing, empty catalogue as a successful
read, malformed body as an error, remote endpoint refused.

The `pfl-cli models` command was drafted and **reverted** this round: its allowlist edit did not
apply and clippy flagged the unused dispatch, so the CLI surface is not shipped. The library
function and its tests are.
## Item 4: usage records (2026-11-21)

`pfl_workflow::usage_summary(run)` sums checkpoint usage while keeping unknown values distinct
from zero (`input_unknown` / `output_unknown` counts).

`pfl-cli usage --workspace <ws>` prints one row per run: version, workflow, outcome, token totals and
the unknown counts.

Verified on a real workspace: replay runs report `0` tokens with `6/4/4/5` unknown per workflow —
which is honest, because recorded replay reports no token counts. A provider that does report usage
shows real totals there.

Gate with real exit codes: build 0, fmt 0, clippy 0, tests 0.
## Cross-platform CI (2026-11-21)

`native-manual.yml` on `d9ff138`: Windows x64, macOS arm64, Linux x86_64 — all **success**.
Acceptance record updated to this revision: model discovery and usage records now recorded as
passing with their evidence class; Cloud selection and cost confirmation remain open.
## Item 4: cost estimation with a stated basis (2026-11-21)

`pfl_workflow::cost` prices recorded usage at **operator-supplied** rates. It never invents a price:
`Pricing` carries the two rates, and an `Estimate` carries the token counts plus a `basis()` string so
a confirmation can state how the figure was produced.

If any stage's usage is unknown the estimate is `None` — unknown, not `0.0`, because a partial total
understates a bill and zero reads as free. A genuinely zero run with no unknowns still yields a real
`0.0`.

Three unit tests: known usage priced from the supplied rates, unknown usage yielding no estimate,
and zero-with-no-unknowns being a real zero.
## Cross-platform CI (2026-11-21)

`native-manual.yml` on `e980d0f`: Windows x64, macOS arm64, Linux x86_64 — all **success**.
Acceptance record moved to this revision with the cost estimator recorded as passing and the Cloud
selection UI still marked not implemented.
## Item 1 closure: Cloud split-UTF8 defect (2026-11-21)

The Cloud SSE loop buffered decoded text (`pending: String`, `push_str(from_utf8_lossy(&bytes))`), so a
multi-byte character split across two network chunks became U+FFFD. Only the Local NDJSON path had
been byte-buffered.

Fixed: `pending` is a `Vec<u8>`, a new `take_sse_line` extracts bytes up to the newline, and the line
is decoded as a whole with `String::from_utf8`. Invalid UTF-8 is now an error rather than a silent
replacement, and the mid-frame check inspects bytes.

Three tests: the old per-chunk lossy decode is shown to corrupt the character (the defect), split
UTF-8 through `take_sse_line` reassembles as `café 🧪`, and invalid UTF-8 is rejected.

151 workspace tests pass; fmt and clippy clean.
## Licence reconciliation (2026-11-21)

Decision 3: the application is MIT (LICENSE, commit 42fe5e9); prompt templates stay CC BY-SA 4.0 under LICENSE-PROMPTS. The workspace metadata key that said no licence had been selected is removed and replaced with license = MIT. cargo metadata reports license MIT for the workspace, so the metadata no longer contradicts the file.


## Phase A (A2/A3 plumbing, 2026-11-21)

The session actor gained a Cloud mode and an in-memory key. Selecting Cloud builds no provider until a key exists; configuring without one is refused with a message naming the key, and no provider call occurs. A Cloud run requires explicit paid approval; without it the start is refused before any provider work. The key is held only in the actor for the session and is never written to settings, snapshots, events or exports. Two session tests prove both refusals with a provider call count of zero.


## Phase A (A7 Lean profile, 2026-11-21)

A Lean installation refuses Local with the message naming the profile, and does not refuse Cloud: a Lean Cloud run starts, fails at the loopback provider with 503, and is recorded as Failed rather than being stopped by the profile gate. Two tests in consent.rs, using a loopback endpoint that always answers 503 so no real Cloud host is contacted.


## Phase A (A4 cost confirmation text, 2026-11-21)

confirmation_text produces the statement a consent screen shows before a paid run. A run's usage cannot be known in advance, so it states that the cost is unknown until the run reports usage, together with the rates that will be applied; with no configured rates it says only that the cost is unknown. Neither form invents a currency figure. Two tests assert this, including that no dollar sign appears.


## Phase A (A3 masked field support, 2026-11-21)

TextInput gained a masked mode for the Cloud key field. The mask replaces every byte with an asterisk while preserving byte length exactly, so cursor and selection offsets do not move; newlines are kept so wrapping is unchanged. Two tests assert offset preservation, that the secret is not painted, and newline handling. The field is not yet placed on the Setup page.


## Phase A (A3/A4 Cloud UI, 2026-11-21)

The Setup page now carries a Cloud section: a masked session-only key field, a Cloud model field, the honest cost statement from confirmation_text, a Configure Cloud button that sends the key to the session actor and configures the mode, and a per-run paid approval control. The run start refuses without that approval, so declining sends nothing. The UI harness checks the key field and the approval control (33 checks). The cost sentence is plain text that the accessibility tree does not expose, so it is covered by unit tests rather than an automation check.


## Phase A (A6 no fallback, 2026-11-21)

A failed Cloud call makes exactly one request (mock counter) and records no checkpoint; the service holds one provider, so there is no second destination to fall back to and the test asserts the count is 1 rather than 6.

A loopback-only verification hook (PFL_CLOUD_ENDPOINT) was added so installed acceptance can stand a local mock in for the Cloud host and count requests without spending. A non-loopback value is refused at provider construction, so the variable cannot redirect a real key off the machine.


## Phase A (A5 proof scaffolding, 2026-11-21)

scripts/mock-cloud.ps1 serves a valid SSE stream on loopback and logs every request; scripts/installed-cloud-consent.ps1 drives the app to configure Cloud, decline, then approve. The decline half currently reports zero requests, but the run is not yet trustworthy: two probes showed the Cloud fields intermittently absent from the automation tree and the configure reply not observed, so the approve half is unproven. A5 stays open until the script waits for elements, confirms the Configured reply, and both halves pass repeatedly.


## Phase A (A5 diagnosis, 2026-11-21)

The Cloud acceptance script exposed two real defects, both now understood. First, configuring Cloud from the UI sent the key and the mode as two separate requests and the mode was observed as unset at start. Second, the UI send() helper discards the reply, so Reply::Configured never reached the view and the mode stayed None even when the actor had configured it. The actor now has one atomic ConfigureCloud request that installs the key and configures the mode together. Applying the reply in the view is still outstanding; the first attempt did not compile and was reverted rather than left broken. A5 therefore remains open, and the proof script is recorded as not passing.


## Phase A (A5 root cause, 2026-11-21)

Geometry, not timing, explains why the Cloud configure never fired. At the default window size the Setup page renders the Cloud card below the viewport: the 'Configure Cloud with this key' button reports a rectangle at y=1260 while the window rectangle ends at y=1227. An Invoke on an element outside the window does nothing, which is why the notice never changed and the mode stayed unset. The Setup content area does scroll (main-content carries overflow_y_scroll), so a user can reach the card; the automation did not scroll, which is a harness defect rather than a product one. The earlier 'pass (33 checks)' row was wrong -- those checks asserted presence in the automation tree, not reachability -- and is corrected to partial.


## Phase A (A5 partial progress, 2026-11-21)

Scrolling the Setup content area with a real mouse wheel clears the clipping problem: the Cloud card is reached and 'Execution mode configured' is now observed, so the mode really is set. Two checks still fail: the Start invoke on the Analysis page does not change the status notice, and no provider request is seen, so the decline and approve halves remain unproven. A5 stays open.


## Phase A (A5 rebuilt proof, 2026-11-21)

scripts/cloud-consent-proof.ps1 replaces the patched harness with helpers copied from ui-acceptance: name-first resolution, wait-for-window, wheel scrolling. It proves Cloud configures against the loopback endpoint (the Configured notice is observed), that synthetic data is declared and that paid approval can be granted. The Start invoke still does not fire, so the decline and approve halves remain vacuous: zero requests is a zero of not starting. A5 stays open.


## Phase A (A5 diagnosis: Start control resolution, 2026-11-21)

A direct tree dump of the Analysis page showed no Start control at all until a workflow tab is selected; adding the tab click made the control resolvable. It still does not fire: the resolved element has no InvokePattern, and the focus fallback fails with 'Target element cannot receive focus'. The installed decline/approve pair is therefore still unproven. Session-level tests already prove both refusals with zero provider calls; the remaining gap is purely automation of the same path through the window.


## Phase A (A5: keyboard start added, proof still blocked, 2026-11-21)

Added a Ctrl+Enter binding (StartRun) so starting a workflow has a pointer-free path; the Start div exposes no InvokePattern, so this is an accessibility improvement in its own right. Driving the proof by that key did not change the status notice either, so the installed decline/approve pair remains unproven and A5 stays open. The proof script was reverted to its committed state rather than left half-edited.


## Phase A (A5 status, 2026-11-21)

The installed decline/approve pair is recorded as blocked on automation, not as passing. Evidence by layer: session tests prove a declined Cloud run is refused with zero provider calls; engine tests prove one request and no fallback; UI automation proves Cloud configures, synthetic data is declared and paid approval can be granted. What cannot be automated is activating Start: the control is a div with role Button but no InvokePattern, and a Ctrl+Enter binding added for the purpose did not fire under UIA-driven focus. A manual click-path checklist is left for this one step: configure Cloud, decline, confirm zero requests, approve, confirm the run proceeds.


## Phase B (B6 revisions list, 2026-11-21)

pfl-cli revisions prints every stored revision for a workspace with its kind, id, version, base run, note and creation time. Verified end to end: pipeline created a claims run, save-revision stored version 1 against it, and revisions listed exactly that record.


## Phase B (B2 diff engine, 2026-11-21)

pfl_workflow::diff computes a deterministic line diff between two revision texts using a longest-common-subsequence table, with added/removed counts and a compact render for the CLI or a review screen. Six tests: identical texts (no phantom line from a trailing newline), insertion, deletion, replacement, determinism of both the structure and the rendering, and empty inputs. No timestamps or hashing, so the same two texts always produce the same output.


## Phase B (B4 revision HTML export, 2026-11-21)

The package now emits claims.html and application.html alongside the markdown, each escaping the revision text into a pre block and naming the revision id and version. A test asserts escaping (angle brackets, ampersand, quotes), that raw markup cannot leak, that re-exporting the same revision produces identical bytes, and that the manifest carries the html file hash.


## Phase B (B4 revision Word export, 2026-11-21)

claims.docx and application.docx join the package, built from the revision text with the same heading mapping as the run exporter and titled with kind, version and revision id. A test asserts the file is a real docx container (PK zip magic, over 1 KB) and that the manifest names it.


## Phase B (B5 semantics + invariant, 2026-11-21)

docs/REGENERATION.md fixes the contract: a regeneration derives a new run from the base snapshot, records lineage, and never mutates the base run or consumes revisions; section scope re-runs named stages and carries the rest over from committed checkpoints. A test pins the whole-run half: rerun yields a new id, the next version, rerun_of set, and the base run's checkpoints, report and version are byte-identical afterwards; the revision survives unchanged.


## Phase B (B5 stage hook, 2026-11-21)

regenerable_stages lists the committed checkpoints of a terminal run, so a section selection can only name stages the base run actually produced; a running run is refused. Tested against a completed feasibility run and against a run still in progress.


## Phase B (B5 plan validation, 2026-11-21)

plan_regeneration validates a section selection against a completed run: an empty selection is refused, a stage the base never produced is refused (probed with a claims stage against a feasibility run), and a valid selection is returned in the base run's own order with duplicates removed, so the plan is deterministic. Execution with carried-over checkpoints is still to build.


## Coverage matrix added (2026-11-21)

docs/ACCEPTANCE.md now carries a directive-wide matrix: every A–G item with its status and the evidence class that proves it, or the concrete gap. Blocked rows name their blocker (automation for A5, authorization for E2, hardware for F3).


## Phase C (C1 PatentsView adapter, 2026-11-21)

PatentsViewSource posts the documented query to the official search endpoint with X-Api-Key, requests only id, title, date and assignee, and maps records to verified PriorArtItems. A record with neither id nor title is dropped rather than invented; a 403 or malformed body is an error, never an empty result; an empty key is refused at construction. Three tests against a loopback mock; no live query was made and none is claimed.


## Phase C (C2 opt-in research wired into a run, 2026-11-21)

With a PatentsView source attached, a run that authorizes research performs exactly one query and records the verified reference in its immutable input; a run without opt-in performs none. The test also exposed a real defect: a fresh feasibility run inherited the reference set of the previous feasibility run, attaching references the operator never chose for that run. upstream() now attaches prior art only for dependent workflows. A failed query fails the run rather than completing with an empty set.


## Phase C (C5 desktop research entry, 2026-11-21)

Setup now carries a masked PatentsView key field, a Configure control that builds the research source in the session actor, and a per-run Use PatentsView research toggle that sets the run's research authorization. The key is session-only, and a loopback override (PFL_RESEARCH_ENDPOINT) is refused for anything but loopback so a real key cannot be redirected. UI harness still passes 33 checks. No live query is claimed.


## Phase B (B2 diff CLI, 2026-11-21)

pfl-cli diff --workspace --a --b renders the deterministic diff between two stored revisions, with added/removed counts, and refuses revisions from different projects. Verified end to end: two revisions saved against the claims run, then diffed, producing exactly one removal and two additions.


## Phase F (F5 Cloud documentation agreement, 2026-11-21)

Phase A made Cloud selectable in the desktop, which made the README and manual statements that it is unavailable wrong. Both now describe the session-only key, the per-run paid approval and the unknown-until-usage cost line, and the Lean profile is recorded as usable for Cloud and replay. This is the documentation-agreement work F5 requires, applied to the one claim the code had already invalidated.


## Phase F (F5 research documentation, 2026-11-21)

The manual still said research was disabled and live retrieval was only a seam. It now describes PatentsView with a session-only key and per-run opt-in, states that a failed query is an error rather than an empty result, and records plainly that no live query has been verified from this build.


## Button automation investigated (2026-11-21)

The premise was wrong. With a native build the Start control is exposed as ControlType.Button with InvokePattern, and installed-local-proof.ps1 invokes it successfully five times (5/5 pass). The earlier 'no InvokePattern' and 'cannot receive focus' findings came from a non-native stub binary that a workspace test had written over the native build; the stub exits immediately with 'requires a native build'. A guard was added to the Cloud proof, and the trap is recorded in HANDOFF. A5 is therefore partial, not blocked on the UI toolkit: the Cloud decline script itself still needs fixing.


## Phase A (A5 closed, 2026-11-21)

The Cloud consent proof now passes 7/7, twice in a row, against the native build and a loopback mock: Cloud configures with a session-only key, declining sends zero requests and names the missing paid approval, granting approval lets the run reach the provider (6 requests for the feasibility run) and complete.

Four script defects had to be fixed, none of them product defects: the stub-binary guard launched a window instead of probing with an invalid page; nav buttons expose no InvokePattern and need a foregrounded coordinate click; Invoke and clicks both do nothing on an element outside the window, so every action now scrolls it into view; and the completion check read the status bar instead of the run summary. The earlier 'no InvokePattern' diagnosis was wrong -- a workspace build had overwritten the native binary with a stub.


## Corrections after review (2026-11-21)

Three corrections, all accepted. (1) The acceptance row for Cloud selection was wrong in the opposite direction: the content area scrolls (overflow_y_scroll), so the card is below the fold, not unreachable, and its y=1260 measurement was stale. The row now records pass, with the scrolling stated. (2) Repetition was unverifiable because results went to a temp folder; the proof now writes its JSON and request log into docs/evidence/cloud-consent/<timestamp>/, and two passing runs are committed there. (3) Action patterns were measured one control at a time: nav items (nav-projects..nav-settings) are ControlType.Button with NO supported pattern, the workflow tab is a TabItem with SelectionItem, and Start new version is a Button with InvokePattern. So the inconsistency is real in the accessibility tree, not a wrong conclusion. Registering on_a11y_action(Click) on nav items did not change it, and removing accessibility_id lost the AutomationId without adding an action, so both experiments were reverted; automation keeps the foregrounded coordinate click for navigation.


## Second review pass: two contradictions removed (2026-11-21)

(1) ACCEPTANCE still carried a 'blocked -- automation cannot activate the Start control' row two lines under the new pass row; the committed evidence disproves it, so that row now reads pass and points at the two evidence folders. (2) REGENERATION.md argued with itself: the header called the note 'the contract the section-scoped version must satisfy' and a section heading still said 'Section scope (the part still to build)' while line 3 closed that option. The note is rewritten for suffix semantics, the heading is 'Suffix scope (the part still to build)', and an invariant row for byte-identical carried stages is added as pending the implementation. (3) The nav-item accessibility gap is now a tracked row in the acceptance matrix, not only prose in this log.


## Phase B (B1 session command, 2026-11-22)

SaveRevision and ListRevisions are now session requests. SaveRevision resolves the base run itself -- the latest completed run of that document kind -- and refuses an empty edit; ListRevisions returns the stored series. The test drives feasibility and claims to terminal with a reply-draining helper that folds drained replies into state, which is the fix for the earlier attempt that raced until() and panicked on intermediate errors: revisions come back as version 1 and 2, both anchored to the claims run, listed in order, with the empty edit refused.


## Gate correction after CI failure (2026-11-22)

CI failed on 0d0a364 because the native view matched on Reply without the new Revision/Revisions arms: my local gate never compiled --features native, so a whole feature-gated module went unchecked. The arms are added (revisions are held in the view and announced), and two pre-existing native-only clippy lints are fixed. The gate now includes cargo clippy -p pfl-desktop --features native -D warnings.


## Phase B (B1 editing surface, 2026-11-22)

The Analysis page now carries the editions panel for the Claims and Application workflows: a multiline revision field, Load run text (seeds it from the run's final stage), Save revision (sends SaveRevision with the workflow's document kind) and List revisions. The UI harness reaches the panel and confirms all four controls (37/37). Also fixed: ui-acceptance's trap called Clear-Harness, which is not defined when the early stub guard throws, so a failed run crashed while reporting; cleanup is now inline.


## Phase B (B1 end-to-end in the app, 2026-11-22)

scripts/editing-proof.ps1 drives the installed app: Load run text seeds the editor from the claims run's final stage, Save revision stores v1, the text is replaced, Save revision stores v2, and List revisions announces two stored revisions. 4/4, twice, with JSON and screenshots under docs/evidence/editing/.

The first attempt is kept deliberately: it failed only because the check looked for the v1/v2 pills in the automation tree, and those rows are plain divs that are not exposed. The screenshot from that attempt shows both rows with their base run and note, and the check now asserts the announced count instead.


## CI flake fixed (2026-11-22)

CI on ec56d13 failed only on macOS: the quit-guard test asserted the active-job count immediately after the Run reply, and on that runner the counter lagged by a scheduling tick. The test now waits for the count, as its neighbouring assertions already did.


## Phase B (B2 diff in the window, 2026-11-22)

Each revision row now carries 'Compare vN with v(N-1)'. The comparison is computed with the deterministic diff engine, announced as 'Diff v1 to v2: 1 added, 1 removed', and rendered inline with +/- markers. The editing proof extends to five checks and passes twice; evidence under docs/evidence/editing/.

Build hygiene: the native UI now builds into target/native, separate from the workspace target, because three times a test run in the same shell overwrote target/session with the non-native stub.


## Phase B (B3 selection from the window, 2026-11-22)

Each revision row offers 'Use vN for compliance'; the Compliance page shows what will be reviewed and starts the run through StartSelected. A session test proves the selected claims revision id and its text are recorded in the run's immutable input. The installed proof reaches eight checks and passes twice: it selects v2, starts compliance, and the screenshot shows a NEW run -- v5 compliance, 4/4, Completed -- with the panel reading 'Claims: the selected claims revision'.


## Phase B (B4 exports, 2026-11-22)

The revision panel exports the selected revisions as a package into the data directory's exports folder, and the proof confirms six files on disk: claims.md, claims.html, claims.docx, references.csv, compliance-findings.md and manifest.json. 11/11, twice.

Word determinism, measured rather than assumed: two separate pfl-cli processes building the same revision produced byte-identical claims.docx (sha256 c7193ae08ba93df1...), claims.html, claims.md, references.csv and manifest.json. A unit test now asserts docx byte-equality for the same revision, and a first measurement across a second boundary was also identical, so docx-rs writes no volatile entry metadata here.


## Phase B (B5 suffix regeneration, 2026-11-22)

Regeneration is suffix-only, as decided: a new run re-runs the chosen stage and every later stage, carrying the earlier ones from the base run verbatim so they stay byte-identical. No copy in the UI or docs promises single-stage regeneration; the buttons read 'Regenerate from <stage> onward'.

Unit test: regenerating from the fourth stage yields a new id and version with rerun_of set, every carried stage identical to the base run, the base run and its revision untouched. Installed proof: the window regenerates the compliance run from Definiteness onward, producing version 6 from base version 5 and completing (12/12, twice).


## Phase C (research end to end, 2026-11-22)

Against a loopback PatentsView mock: a run without the per-run opt-in completes and the mock records zero requests; enabling the opt-in installs the key and the run sends exactly one query; the verified reference is then rendered on the run from its immutable input, with an addressable row ('10123456 - Container with integrated temperature indicator - Acme Sensors - 2022-05-17 - USPTO PatentsView - verified'). 8/8, twice, with JSON, request log and screenshot per run under docs/evidence/research/.

The live query remains blocked on a key; nothing here contacts a real provider. Two fixes came out of this work: enabling the opt-in now installs the key (so a run cannot be authorized for research that was never configured), and a research configuration has its own reply rather than reusing the mode notice.

