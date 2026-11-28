# Product status

Last updated: 2026-11-24. Windows x86_64 installed acceptance; macOS and Linux artifacts only.

This page is a pointer, not a second inventory. **`docs/ACCEPTANCE.md` is the record**: one row
per requirement with its evidence class, the blocked items and their owners, and the decisions
that remain open. A restated summary here would drift from it, which is exactly what happened
before: this file claimed a dated test count and said research was a fixture long after the
adapter, the consent gate and the live-query path existed.

What this page still answers:

- **The product's shape**: a stand-alone Rust core with a GPUI desktop, a CLI harness and a
  compatibility HTTP API, covering four analyses over one invention with immutable runs.
- **What a release decision rests on**: the matrix and the release manifest, not this page.
- **Where the detail lives**: `docs/HANDOFF.md` for a fresh session, `docs/BUILD-EVIDENCE.md`
  for the chronological trail, `docs/RELEASE-MANIFEST.md` for artifacts and hashes.
## Implemented and verified

| Area | State |
| --- | --- |
| Domain contracts | 19 stages across four workflows, immutable run inputs, upstream snapshots, typed IDs, durable events. |
| Storage | Transactional SQLite, idempotent versions, atomic checkpoint/usage/event writes, cancellation and stale-attempt guards, synthetic legacy importer with backup/verify/activate, OS-lock-protected profiles. |
| Feasibility workflow | 6 stages, prompts byte-identical to the frozen legacy source. |
| Claims workflow | 4 stages (plan, draft, examine, revise) using the legacy claim-drafter prompts. |
| Compliance workflow | 4 checks (written description, definiteness, formalities, eligibility) using the legacy compliance prompts. |
| Application workflow | 5 sections plus a non-LLM IDS/assembly step using the legacy application prompts. |
| Exports | Markdown, HTML and real `.docx`, all from the same saved run identity. |
| Report reading | Reports render as documents (heading hierarchy, emphasised labels, bordered tables, monospaced code) with a source view for selection and copying. Parser is unit-tested. |
| Consent | Every run records whether it carried synthetic test material or a real invention; replay refuses real data, and local mode refuses a non-loopback endpoint. |
| Desktop | Native GPUI interface with a layered dark design system: icon rail, elevated cards with real box shadows, workflow tabs, outcome pills, checkpoint progress bars, stage ledger, document pane, hover states, Ctrl+1–6 page navigation. Projects, intake with nine context fields, live progress, cancel/resume, history, reports, three export formats, setup-profile selection, single-owner lock, app-wide quit guard. |
| Accessibility | A real AccessKit tree is published: labelled buttons, text inputs, tabs, headings, and status regions with stable AutomationIds. Screen readers get structure, and `scripts/ui-acceptance.ps1` drives the window through UI Automation (20 checks, all passing). |
| CLI | `replay`, `import-replay`, `pause-fixture`, `resume`, `pipeline` (all four workflows), `serve`. |
| API | Compatibility auth (Bearer, legacy 401 messages, tokenless mode, single-use 5-minute download tokens, long-lived query tokens), health shape, project/run/event/report/export routes. |
| Providers | Recorded replay, Ollama over NDJSON (loopback only), Anthropic Messages over SSE. Each fails closed without its exact snapshot and explicit authorization; a stream missing its terminal frame is recorded as **Truncated**, never Completed. |
| Live local inference | Verified 2026-11-21 with `phi4-mini:latest` on the running local Ollama: **all four workflows, 19 stages, 19 real calls, 715.5 s**, then reopened with zero regeneration. |
| Research | Opt-in USPTO PatentsView source with a session-only key, a per-run consent gate and an offline fixture; supplied references are snapshotted into the run input. No live query has been verified. |
| Setup profiles | Lean is Cloud-only and refuses a local runtime; Full permits both. |

Test suite: **96 passing tests** across the workspace, plus `cargo fmt --check`
and `cargo clippy --all-targets -D warnings` clean. Provider wire formats are
verified against local mock servers, and `scripts/ui-acceptance.ps1` adds 20
automated UI checks against the real native window.

## Not done, and not claimed

| Gap | Why it is open |
| --- | --- |
| Native window never launched | **No longer true**: the window has been launched and every page rendered (Projects, Intake, Analysis, Report, History, Setup). What remains unverified is OS input behaviour — typing, IME, clipboard, the native save dialog, and GPU behaviour on other drivers. Automated click injection does not reach the GPUI window from this session, so interactive checks still need a human. |
| Only one live model tested | `phi4-mini:latest` on this machine. Larger models and output quality across a wider sample are unmeasured. The live run also invented prior-art reference numbers, which is expected without a research source. |
| Paid Cloud call never executed | Deliberately fail-closed; a real call needs an API key and explicit cost approval. |
| Live research provider | Requires an external prior-art API and its terms; only the seam and fixture exist. |
| macOS and Linux | **Compile and test green in CI** (run 44950601970): fmt, tests, clippy and native build pass on macOS arm64 and Linux x86_64. No installed package, launched window or interactive acceptance on either platform. |
| Packaging, signing, notarization | Windows installer built and install-accepted (7.15 MB, per-user, uninstall preserves history) but **unsigned**; signing policy still undecided. No macOS or Linux package. |
| Full data migration of a real PatentForge database | Only the synthetic fixture importer exists; real migration needs explicit authorization. |
| Legacy tray lifecycle and DSH integration | Lite closes when the window closes and refuses a second instance; there is no tray-resident mode. |

## Suggested next steps

1. Authorize one isolated native launch and drive the checklist in
   `FIRST-SLICE-ACCEPTANCE.md` by hand.
2. Select a specific installed local model and run the same journey live,
   keeping replay and live results separate.
3. Decide the packaging/signing policy, then build installers per platform.
4. Only after that, consider real-data migration and any API cutover.

## Watch items carried forward

These are real and unresolved. None is a defect in a shipped path; each is a decision or a
measurement that has not been made.

- **Model rates in `cost.rs`** (Claude Opus 15/75, Sonnet 3/15, Haiku 0.8/4 per million tokens) were
  entered some time ago and not re-checked against current published prices. The approval screen
  presents them as "Rates applied", so confirm them before any paid release.
- **Every read runs in an `IMMEDIATE` transaction** with a five-second busy timeout
  (`crates/pfl-storage/src/lib.rs`). Several pollers on one workspace could contend and hit
  "database is locked". Not observed under the harness; not measured under load.
- **Each keystroke clones the document** into a 200-entry undo stack and shifts with
  `Vec::remove(0)` (`crates/pfl-desktop/src/editor.rs`). Long application drafts will be slower
  and heavier than they need to be.
- **The audit's escalation step was never taken**: it recommended an audit-team pass, and that
  skill is not installed on this machine. The second full review was done by an external reviewer
  instead.
- **Version strings agree**: the crates, the installer and the manifest all declare 0.2.1, and the
  only published release is v0.1.0. The skew that used to be recorded here is closed.