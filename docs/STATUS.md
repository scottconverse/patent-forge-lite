# Product status

Last updated: 2026-11-21. Windows x86_64.

Patent Forge Lite is a stand-alone Rust core with a GPUI desktop, a CLI harness
and a compatibility HTTP API. It is **not** a finished shipping product. This page
is the honest inventory.

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
| Desktop | Native GPUI interface with a layered dark design system: icon rail, elevated cards with real box shadows, workflow tabs, outcome pills, checkpoint progress bars, stage ledger, document pane, hover states, Ctrl+1–6 page navigation. Projects, intake with nine context fields, live progress, cancel/resume, history, reports, three export formats, setup-profile selection, single-owner lock, app-wide quit guard. |
| Accessibility | A real AccessKit tree is published: labelled buttons, text inputs, tabs, headings, and status regions with stable AutomationIds. Screen readers get structure, and `scripts/ui-acceptance.ps1` drives the window through UI Automation (20 checks, all passing). |
| CLI | `replay`, `import-replay`, `pause-fixture`, `resume`, `pipeline` (all four workflows), `serve`. |
| API | Compatibility auth (Bearer, legacy 401 messages, tokenless mode, single-use 5-minute download tokens, long-lived query tokens), health shape, project/run/event/report/export routes. |
| Providers | Recorded replay, Ollama over NDJSON streaming, Anthropic Messages over SSE. Each fails closed without its exact snapshot and explicit authorization. |
| Live local inference | Verified 2026-11-21 with `phi4-mini:latest` on the running local Ollama: **all four workflows, 19 stages, 19 real calls, 715.5 s**, then reopened with zero regeneration. |
| Research | Injectable `ResearchSource` seam with an offline fixture implementation; supplied references are snapshotted and reach the prior-art prompt. |
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
| macOS and Linux | No runners on this machine; the manual CI workflow exists but has never been dispatched. |
| Packaging, signing, notarization | Not started. Requires policy decisions and certificates. |
| Full data migration of a real PatentForge database | Only the synthetic fixture importer exists; real migration needs explicit authorization. |
| Legacy tray lifecycle and DSH integration | Not implemented; Lite has no tray-resident background mode. |

## Suggested next steps

1. Authorize one isolated native launch and drive the checklist in
   `FIRST-SLICE-ACCEPTANCE.md` by hand.
2. Select a specific installed local model and run the same journey live,
   keeping replay and live results separate.
3. Decide the packaging/signing policy, then build installers per platform.
4. Only after that, consider real-data migration and any API cutover.
