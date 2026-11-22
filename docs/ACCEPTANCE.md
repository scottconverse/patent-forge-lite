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
| Revision | `cc92d2a` |
| Candidate version | 0.2.0 (crates still declare 0.1.0 — see `docs/RELEASE-MANIFEST.md`) |
| Published release | v0.1.0 only; this revision is **unpublished and unsigned** |

## Summary

| Status | Count |
| --- | --- |
| pass | 54 |
| not verified | 1 |
| blocked — hardware | 2 |
| blocked — key | 1 |
| blocked — authorization | 1 |
| open defect | 2 |

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
| Paid approval control present in the window; run refused without it | installed | pass |
| Declining sends zero requests; approval reaches the provider | installed + mock | **open** — the proof passed 7/7 twice at earlier revisions but fails 3 of its checks at this one after the Setup page grew; invoking the configure control directly still returns its own error, so the control works and the automation does not. Evidence for the earlier passes remains in `docs/evidence/cloud-consent/`. |
| No fallback: a failed Cloud call makes exactly one request and leaves no checkpoint | unit, `consent.rs` | pass |
| Loopback-only verification hook; a non-loopback Cloud endpoint is refused | unit | pass |
| Lean refuses a local runtime with a clear message | unit, `consent.rs` | pass |
| Lean permits Cloud past the profile gate (fails at the provider, not the gate) | unit, loopback 503 | pass |
| Masked key field; the mask preserves byte offsets | unit | pass |
| Cost line states *unknown* plus the rates, never a fabricated total | unit, `cost.rs` | pass |
| Cost line displayed in the window before a paid run | installed + mock, 11/11 twice | pass — 11/11 run also asserts `3.00 input / 15.00 output` |

## Phase B — a product a person can use

| Item | Evidence | Result |
| --- | --- | --- |
| Revisions versioned and anchored to immutable runs | integration | pass |
| Compliance reviews a selected revision rather than the latest | integration | pass |
| Claim structure validation (numbering, dependencies, references) | unit, CLI run | pass |
| Edit → validate → save → package chain | run | pass |
| `SaveRevision` / `ListRevisions` session commands, anchored to the latest completed run | unit, reply-draining helper | pass |
| Editing surface: revision field, Load run text, Save revision, List revisions | installed, harness 37/37 | pass |
| Editing end to end: load text, save v1, edit, save v2, both listed | installed, 4/4 twice | pass — `docs/evidence/editing/` |
| Deterministic line diff (LCS) | unit, 6 tests | pass |
| `pfl-cli diff` between two stored revisions | run | pass |
| Revision rows with a compare action and an inline diff | installed, 5/5 twice | pass — announces `Diff v1 to v2: 1 added, 1 removed` |
| Select revisions for compliance from the window; the run records them | installed, 8/8 twice; session test for the recorded id | pass — new run `v5 · compliance, Completed` |
| Export selected revisions (Markdown, HTML, Word, CSV, manifest) | installed, 11/11 twice | pass — six files written and listed |
| Word export byte-stability | two separate processes | pass — identical `claims.docx` and every other file |
| Regeneration semantics and plan validation | doc + unit, 5 tests | pass — `docs/REGENERATION.md` |
| Execute "regenerate from here onward"; earlier stages byte-identical | unit + installed, 12/12 twice | pass — v5 regenerated as v6, Completed |
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
| Copy-first importer: fixture, preview counts, provenance warnings, preserved archive, re-runnable, source untouched, missing source refused | run, 15/15 twice | pass — `docs/evidence/migration/` |
| Credentials redacted at import (column recorded, value never carried); re-entered per session | unit + run | pass — the proof scans the activated database bytes for the fixture's key material |
| Read API contracts verified over loopback; CLI matrix and DSH compatibility documented | run, 13/13 twice | pass — `docs/API-CONTRACTS.md`, `docs/evidence/api/` |
| HTTP write endpoints | — | **not verified** (they change state; nothing exercised them) |
| A real PatentForge database | — | **blocked — authorization and a copy** |

## Phase F — packaging

| Item | Evidence | Result |
| --- | --- | --- |
| One Windows installer from one revision, both profiles, SHA-256 recorded | run | pass — `docs/RELEASE-MANIFEST.md`, revision `27dbd52`, `5fc41eb0…` |
| Windows install, upgrade, rollback, uninstall, data retained | run, 11/11 twice | pass — `docs/evidence/upgrade/` |
| Signature status recorded truthfully | review + `Get-AuthenticodeSignature` | pass — v0.1.0 signed; candidate unsigned |
| Licence metadata agrees with the committed licence | run, `cargo metadata` reports MIT | pass |
| macOS and Linux artifacts from this revision | — | **blocked — hardware**; runbook delivered |
| macOS and Linux installed acceptance | — | **blocked — hardware** — `docs/PLATFORM-RUNBOOK.md` |
| Documentation agreement at this revision | review | pass — manual, packaging, release notes and this matrix describe what the build does |
| This matrix checks itself, in CI: counts match rows, evidence paths exist, no stale claims, no single-stage promise | run (`scripts/acceptance-check.ps1`) + CI step | pass — verified to fail on an injected bad count, a missing evidence path and a stale claim |

## Open defects

| Defect | Measurement | Status |
| --- | --- | --- |
| Nav items are announced as buttons with no accessible action, so assistive technology cannot press them; automation compensates with a foregrounded coordinate click | measured per control: `nav-*` patterns `[]`, workflow tab `SelectionItem`, Start `Invoke` | **open** — registering `on_a11y_action(Click)` did not change it, and dropping `accessibility_id` lost the `AutomationId` |

## Decisions that remain Scott's

1. **Next release version.** The installer says 0.2.0 by compile-time override while the
   crates say 0.1.0. Either bump the crates and re-issue, or keep 0.1.0 and accept that a
   later revision would reuse a published file name.
2. **Signing.** Nothing in this candidate is signed; signing a new revision needs approval
   and a certificate.
3. **Publication.** The candidate is unpublished. Publishing, tagging and release notes
   for 0.2.0 are all still to be decided.
4. **PatentsView key.** One live query would close the last Phase C row.
5. **Migration authorization and a real database copy.** That closes Phase E2.
6. **macOS and Linux hardware.** Without it those rows stay blocked, however good the
   runbook is.
7. **The `impl/*` branches.** Keep or prune; deciding is not reversible without their SHAs.

## What this document does not claim

- That the candidate is installed acceptance on macOS or Linux.
- That any live provider has been contacted.
- That a real legacy database has been migrated.
- That the HTTP write surface works.
- That the accessibility tree is complete, given the open nav defect.
