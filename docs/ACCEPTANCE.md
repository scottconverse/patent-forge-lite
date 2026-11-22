# Acceptance record

Cloud consent is designed but not implemented: see [CLOUD-CONSENT-DESIGN.md](CLOUD-CONSENT-DESIGN.md). The
key-storage decision in that note is open and blocks the implementable half.

Revision: `e980d0f` · Candidate: `dist\patent-forge-lite-0.1.0-x64.exe` (unsigned, unpublished)
Record owner: agent · Status: **incomplete** — several gate items below are open by design.

This record separates what was **executed** from what is merely implemented. Anything not run is
listed as not run, not as passing.

## Evidence classes

- **Unit / integration**: `cargo test --workspace` in this repository.
- **CI**: `.github/workflows/native-manual.yml` on a pushed commit, one job per platform.
- **Run**: a command executed in this session with its output quoted.
- **Installed**: the packaged application installed and driven through UI Automation.
- **Mock**: exercised against a loopback server standing in for a provider.

These are never interchangeable. Replay and mock evidence proves orchestration, not research
quality or legal correctness.

## 1. Engine and storage

| Item | Evidence | Result |
| --- | --- | --- |
| Four workflows, 19 stages, verbatim upstream prompts | unit + CLI `pipeline` | pass |
| Transactional SQLite, idempotent versions, checkpoint/attempt guards | unit | pass |
| Crash recovery across fresh processes | integration (`fresh_process`) | pass |
| Single-owner profile lock | integration | pass |
| Exports: Markdown, HTML, `.docx` | unit; Word opened by hand (Scott) | pass |

## 2. Providers and consent

| Item | Evidence | Result |
| --- | --- | --- |
| Replay refuses real-invention declaration | unit | pass |
| Exactly one data declaration (service and local adapter) | unit | pass |
| Real data refused for Cloud (local-only in this build) | unit | pass |
| Local endpoint must be loopback | unit + CLI run | pass |
| Terminal frame required; missing frame → `Truncated` | unit + installed run | pass |
| Stop reason allowlist (`stop` / `end_turn` / `stop_sequence`) | unit | pass |
| Output-limit stop → `Truncated`, no report | unit + installed run | pass |
| Malformed / mid-frame cloud stream → error | unit | pass |
| Split UTF-8 across SSE chunks reassembles; invalid UTF-8 is an error, not U+FFFD | unit | pass |
| Cloud mode at the session layer; key held in memory for the session only | unit (session tests) | pass |
| Cloud start refused without explicit paid approval, zero provider calls | unit (session tests) | pass |
| Cloud selectable in the desktop UI | installed, UI automation | **partial** — controls exist but the Cloud card is clipped below the window (button at y=1260, window ends y=1227) |
| Paid Cloud approval control present; run refused without it | installed, UI automation | pass |
| Declining sends zero requests in the installed app | — | **blocked — automation cannot activate the Start control** |
| No fallback: a failed Cloud call makes exactly one request, no checkpoint | unit (`consent.rs`) | pass |
| Loopback-only verification hook for the Cloud endpoint (non-loopback refused) | unit (build) + code review | pass |
| Lean refuses Local with a clear message | unit (`consent.rs`) | pass |
| Lean permits Cloud past the profile gate (fails at the provider, not the gate) | unit, loopback 503 | pass |
| Cost confirmation states *unknown* plus the rates, never a fabricated total | unit (`cost.rs`) | pass |
| Cost confirmation shown before a paid run in the desktop | — | **not implemented** |
| Masked input field (mask preserves byte offsets; secret never painted) | unit (`masking_tests`) | pass |

## 3. Reference lineage and research

| Item | Evidence | Result |
| --- | --- | --- |
| References supplied to feasibility reach claims, compliance, application | integration | pass |
| IDS assembled only from recorded records | integration | pass |
| Manual references via CLI (`--references`) | run | pass |
| Opt-in HTTP research adapter, distinct empty/failed outcomes | unit (mock) | pass |
| A real provider contacted | — | **not attempted** (needs provider choice and credentials) |

## 4. Revisions, review and export

| Item | Evidence | Result |
| --- | --- | --- |
| Versioned revisions anchored to immutable runs | integration | pass |
| Compliance reviews selected revisions | integration | pass |
| Claim structure validation (numbering, dependencies, references) | unit + CLI run | pass |
| Edit → validate → save → package chain | run | pass |
| Deterministic line diff between two revisions (LCS) | unit (`diff.rs`) | pass |
| Revision comparison in the desktop | — | **not implemented** |
| `pfl-cli revisions list` (id, kind, version, base run, note) | run | pass |
| Desktop editing surface | — | **not implemented** |

## 5. Desktop and accessibility

| Item | Evidence | Result |
| --- | --- | --- |
| Branded GPUI interface, all pages render | installed, screenshots | pass |
| Accessibility tree, labelled controls, announced run status | installed | pass |
| `scripts/ui-acceptance.ps1` (isolated profile, PID-bound) | installed | pass (31 checks) |
| Real-declared Local start, Truncated display, same-run resume | installed + loopback mock | pass |
| Resume skips completed stages | installed + mock request counts (6 → 3 → 4) | pass |
| macOS / Linux installed behaviour | — | **not run** |
| Full / Lean usable as designed | — | **not run** (Cloud not selectable) |

## 6. Project management, setup, lifecycle

| Item | Evidence | Result |
| --- | --- | --- |
| Projects, intake, optional context fields | installed | pass |
| Local endpoint/model configuration | installed | pass |
| Model readiness / installed-model discovery | unit (mock) + run against the real runtime | pass (36 models) |
| Usage records, unknown ≠ zero | unit + run | pass |
| Cost estimation from operator-supplied rates, basis stated | unit | pass |
| Cloud selection and per-run cost confirmation UI | — | **not implemented** |
| Tray / background lifecycle | — | **not implemented** |

## 7. Migration

| Item | Evidence | Result |
| --- | --- | --- |
| Synthetic legacy importer with backup/verify/activate | integration | pass |
| Real PatentForge data migration | — | **not implemented** (needs explicit authorization) |

## 8. Packaging

| Item | Evidence | Result |
| --- | --- | --- |
| Windows installer built, installed, uninstalled, data retained | installed | pass |
| Azure Trusted Signing of app and installer | Azure run + `Get-AuthenticodeSignature` | pass (v0.1.0) |
| Licence metadata agrees with the committed licence | run (`cargo metadata` reports `"license":"MIT"`) | pass |
| macOS / Linux packages | CI artifacts | built; unsigned; **not installed** |
| Upgrade / rollback | — | **not tested** |
| Full / Lean package variants | — | **not built** |

## Cross-platform CI

`native-manual.yml` on `4c0f05e`: Windows x64, macOS arm64, Linux x86_64 — all **success**
(fmt, workspace tests, clippy `-D warnings`, native build per target).

## Release decision inputs

- No signature or publication was made for this revision.
- Nothing paid was contacted; no private data was used.
- The candidate is unsigned and unpublished; v0.1.0 remains the only published release.

## Directive coverage matrix (v3)

Status per directive item. Evidence classes: unit, run, installed, mock, CI. A row is pass only with the evidence named.

| Item | Status | Evidence or gap |
| --- | --- | --- |
| A1 Cloud split-UTF-8 | pass | unit: 3 tests incl. the defect demonstration |
| A2 Cloud mode at session layer | pass | unit: session tests |
| A3 memory-only key, masked field | pass | unit: masking tests; installed: field present |
| A4 cost confirmation text | pass | unit: 2 tests |
| A5 decline sends zero (installed) | blocked | automation cannot activate Start; session+engine proven |
| A6 no fallback | pass | unit: 1 request, no checkpoint |
| A7 Lean refuses Local | pass | unit: refuses Local, permits Cloud |
| B1 desktop editing to revision | not implemented | engine save_revision exists |
| B2 diff engine | pass | unit: 6 tests |
| `pfl-cli diff` between two revisions | run | pass |
| B2 `pfl-cli diff` between stored revisions | pass | run |
| B2 diff in desktop | not implemented | |
| B3 revision selection in desktop | not implemented | engine selection exists |
| B4 revision exports md/html/docx/csv/manifest | pass | unit: 7 tests |
| B5 semantics + plan | pass | doc + unit: 5 tests |
| B5 execution with carry-over | blocked — design | contiguous-prefix invariant; two options recorded in REGENERATION.md |
| B6 revisions list | pass | run |
| C1 PatentsView adapter | pass (mock) | unit: 3 tests; live query blocked on key |
| C2 opt-in research records verified references; opt-out queries nothing | pass | integration (`research_run.rs`) |
| C3 denied opt-in zero requests | pass | unit: research tests |
| C4 IDS from records only | pass | unit: lineage test |
| C5 desktop research entry: masked key, configure control, per-run opt-in | installed (harness) | partial — controls present, live query blocked on key |
| D1 model picker | partial | list_local_models verified against real runtime; no picker UI |
| D2 first-run profile choice | partial | profile selectable in Setup; no first-run prompt |
| D3 tray/lifecycle | not implemented | |
| D4 Windows upgrade/rollback | not tested | installer exists |
| E1 copy-first importer | partial | synthetic importer tested; real path absent |
| E2 real database import | blocked | authorization + database needed |
| E3 credential re-entry | not implemented | |
| E4 API contract + docs | partial | docs exist; contracts unverified |
| F1 one installer per platform | partial | Windows built; macOS/Linux artifacts built unsigned |
| F2 Windows install/upgrade/uninstall | partial | install/uninstall/data retained proven; upgrade not tested |
| F3 macOS/Linux installed | blocked | hardware |
| F4 signature status | pass | v0.1.0 signed and verified; candidate unsigned by design |
| F5 documentation agreement | partial | Cloud and research claims corrected to match the implemented paths; full pass owed at the release revision |
| G completion matrix | partial | this section; full matrix at release |

