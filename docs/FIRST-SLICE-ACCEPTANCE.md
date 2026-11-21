# First-slice acceptance

## Goal and status

Demonstrate one usable native Patent Forge Lite journey on Windows, macOS and Linux, while keeping PatentForge in place. Establish native builds on all three from the start, rather than completing one platform before considering the others.

This is an acceptance checklist, not a blanket pass. First-slice source implementation is active. The initial 14 portable desktop editor/state tests passed centrally before the nine-field context extension; context/history and real-session tests require their own reruns. See [native UI evidence and limitations](native-ui.md). No app window, live local inference, native interaction or installed acceptance is established by the initial portable tests or the native-dependency-only check.

## Stage 1: deterministic native vertical slice

Use a synthetic invention and recorded provider responses. Fixtures must be clearly labeled and contain no real invention, credential, private configuration or production database content. Record fixture provenance and applicable licensing before adding material.

| Check | Observable result |
| --- | --- |
| Native entry point | A minimal GPUI application builds natively for each target from the same revision. It does not require the legacy application services. |
| Separate identity | Launch uses distinct application identity and writable locations; no existing installation, settings, ports or integrations are claimed or modified. |
| Intake | Create/open a synthetic project, enter and edit invention text, save it, close/reopen the app and recover the same content. |
| Optional context | Edit each of the nine optional invention fields; collapse/reopen them and save a basic-field-only change without erasing untouched context. Preserve original labels/order in all six stages. |
| Text usability | Unicode entry, selection, clipboard, undo, focus traversal and platform shortcuts work. Long report navigation/scrolling remains usable. |
| Analysis | Start the recorded analysis through the normal native command path; show stage progress, streaming and explicit terminal status. Clearly identify replay mode. |
| Navigation | Navigate away and back during the run. The runner continues, the view resubscribes and no duplicate provider request or run is started. |
| Cancellation | Cancel in-flight work, acknowledge cancellation and preserve completed checkpoints and usage. Repeated commands do not create overlapping attempts. |
| Recovery | Interrupt before stage 1 and during a later stage, restart, then resume the same run without repeating completed stages or losing saved input. |
| Saved result | Reopen the persisted report and stage/history views without generating again. Verify the selected saved version is the one displayed. |
| Historical import | With an explicit synthetic import, reopen the older edited report while a newer version progresses. Preserve selected-version display/export and disclose any reconstructed legacy request metadata. |
| Export | Export the saved report through the native interface to the selected destination. Open the actual file and check content, Unicode and version identity. Record exactly which formats were exercised. |
| Setup profiles | Lean reaches Cloud setup without acquiring Ollama. Full offers Local/Cloud and recognizes a fixture-declared existing model without initiating acquisition. |
| Core policy | Navigation/read/export causes zero inference calls. Declined paid approval causes zero paid requests. Local selection cannot silently fall back to Cloud. |
| Lifecycle | Window close, quit, restart and interruption have documented effects. Existing PatentForge, DSH and independently owned Ollama remain untouched. |
| Profile ownership | A second process cannot recover an actively owned Lite database; death of the owning process releases its OS lock and permits recovery on restart. |

Replay verifies orchestration and usability, not live model readiness or output quality. Do not mark live inference passed from this stage.

## Stage 2: real authorized local inference

Only after replay behavior is accepted and the test is explicitly authorized:

1. Agree the on-machine endpoint, installed runtime/model, synthetic input and allowed side effects. A configured remote endpoint is not local-machine evidence.
2. Use the same native entry point, Rust runner, persistence and export path, replacing only the recorded provider adapter.
3. Complete the project-to-report journey, including navigation away/back, saved-result reopening and export. Exercise interruption/resume and cancellation with observable request/checkpoint identities.
4. Verify stage completion, structure, persisted content and real usage where measurable. Do not require live prose to equal a fixture.
5. Confirm no unintended Cloud/research traffic, model acquisition, changes to PatentForge or shutdown of independently owned runtimes.

Report the revision, OS/architecture, Rust/GPUI versions, runtime/model identity, input fixture, checks run, actual outcomes and limitations. Do not record credentials or private machine paths in repository evidence.

## Platform evidence

Keep these results distinct for each platform:

- Native compile/test result.
- Native GPUI interaction on an authorized isolated session.
- Recorded-provider versus live-local result.
- Package creation and package-content inspection.
- Clean installed acceptance, including launch, export, restart, uninstall/data retention and recovery.

Cross-compilation, a headless browser test, a structural DMG check or runner labels do not substitute for native/installed evidence. Respect the user's headless-desktop requirement; obtain approval for an isolated interactive route rather than launching on an occupied desktop.

Missing runners or authorization leave the affected result **not run/blocked**, not passed or removed from scope. Preserve macOS arm64 and Linux x86_64 coverage established by legacy payload evidence. Verify Windows payload targets and resolve macOS support-floor conflicts explicitly; do not silently change platform support.

## Later gates, not scope cuts

The first slice is a delivery order, not the complete product:

- Research, claims, compliance, application drafting, all existing workflow-specific exports, compatibility API and migration remain planned scope.
- Real private-data migration requires separate authorization, copy-first safeguards, interrupted/repeated-import checks and demonstrated restore.
- Cloud, paid and research acceptance requires explicit data/destination/cost authorization. Credentials are not blanket consent.
- Full and Lean remain profiles of one product. Validate both packages on all three OSes from one candidate revision.
- Signing-policy ownership, macOS trust/notarization, any additional CPU coverage and exact support floors require decisions at their affected gates.
- No tag, upload, external release or deprecation/cutover is implied by acceptance. Publication and any future retirement of PatentForge require explicit authorization.

Retain source/prompt licenses, attribution, notices and product disclaimers throughout. Do not import legacy content before its obligations are reviewed. No new source license is chosen by this checklist.
