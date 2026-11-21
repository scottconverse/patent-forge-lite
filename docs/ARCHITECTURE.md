# Architecture

## Product and current boundary

Patent Forge Lite is a separate application and repository that coexists with PatentForge. First-slice implementation is active. Shared contracts live in `pfl-domain`, persistence in `pfl-storage`, durable execution/providers in `pfl-workflow`, and desktop presentation in `pfl-desktop`. Broader research/drafting, compatibility and distribution boundaries below remain staged scope, not claims of implemented parity.

Rust owns persistence, workflow execution, provider policy, research, editing, exports and lifecycle. GPUI owns the native desktop. Windows, macOS and Linux are first-class targets from the first build milestone. Do not introduce a browser-hosted interface or require legacy application services as a delivery shortcut.

## Logical boundaries

| Area | Responsibility |
| --- | --- |
| GPUI desktop | Projects, invention form, setup/settings, analysis progress, report reader, research/details, claim/compliance/application views, input, focus, selection, clipboard, undo and navigation. |
| Application service | Shared typed commands, queries and events for desktop and the future compatibility API. Validation, authorization and idempotency live below the view layer. |
| Workflow runner | Own jobs independently of windows; immutable run inputs, cancellation, checkpointing, resume, attempts, rerun versions and terminal outcomes. |
| Storage | SQLite repositories, versioned schema, durable relationships, history, usage and transactional checkpoints. Keep secrets out of ordinary event payloads and logs. |
| Provider and research adapters | Ollama, authorized Cloud inference and opted-in research. Enforce destinations, data purpose, spend approval, cancellation and explicit known/unknown usage. |
| Import/export adapters | Authorized copy-first legacy import, report/document formats, Unicode and edit/version preservation. |
| Platform adapters | Application paths and identity, single instance, dialogs, credential access, tray/menu, runtime discovery and owned-process shutdown. |
| Compatibility API | Opt-in, authenticated and configurable binding through the same application service, without imposing the old private service topology. |

The coordinator/integration owner controls shared contracts, workspace manifests, lockfile, Git operations and resource-heavy builds. Workers own disjoint modules. The desktop's portable `editor` and `state` modules contain no provider execution; its application-session actor owns a long-lived Tokio runtime and uses the real `WorkflowService`. Navigation changes presentation, not runner ownership.

## Commands, events and recovery

The domain contract identifies project, run, version, attempt and stage. Snapshot invention input, prompt version, provider/model and research policy for each new run. Intake includes all nine legacy optional context fields, not only title/description; collapsed optional controls must preserve their values. Use secret references, not credential material.

Commands cover preparation/estimate, authorized idempotent start, cancellation, checkpoint resume, rerun-as-version and export. Queries cover snapshots, history and saved artifacts. Subscriptions replay sequenced events after a known sequence number and never start or resume work.

Persist a completed checkpoint before announcing its durable completion. Navigation changes subscriptions, not execution ownership. Reopening a project must not issue another model request. Distinguish cancelling, cancelled, failed, interrupted and completed states. An interruption before the first completed stage must remain recoverable.

Closing a window and explicitly quitting are different operations. Quit must checkpoint or mark interruption and stop only application-owned work. Never terminate an independently managed Ollama instance or an existing PatentForge/DSH process.

## Desktop delivery

The first slice covers the project browser, intake/editor, running view, saved report/history, settings/setup and export interaction. Later slices add the remaining research and drafting views without dropping them from scope.

The desktop source uses a custom `EntityInputHandler` backed by a portable grapheme-safe buffer, UTF-16/IME composition, undo/redo and native shaped/wrapped text. Saved reports use the same surface read-only, with selection, copying and document scrolling, not an HTML iframe. Verify actual OS input, focus, clipboard and long-report behavior in an authorized native session; portable tests do not establish that evidence.

Full and Lean will share the same implementation and candidate revision as later distribution work:

- **Lean:** Cloud-first setup, no automatic Ollama acquisition.
- **Full:** Local/Cloud choice, system/model readiness, existing-model reuse and explicitly authorized runtime/model acquisition.

Do not make a Full launch install software, pull a model or send invention data implicitly. Readiness checks must distinguish inspection from authorized side effects.

## Coexistence and data safety

Select distinct application/bundle identifiers, install destinations, writable directories and instance ownership. Select API binding deliberately; never reuse or change existing ports automatically.

The first desktop uses application ID `com.scottconverse.patentforgelite` and a dedicated `patent-forge-lite` directory under the platform's per-user data base. A held OS file lock, not a stale create-only sentinel, excludes another owner of the same Lite profile before startup recovery. It is unrelated to legacy locks and ports.

Legacy import is an explicit later operation: authorized consistent backup including committed WAL data, migration of a separate copy, a schema ledger, validation, then atomic activation. Repeating or interrupting import must preserve a usable original and avoid duplicate records. Preserve relationships, history, edits, settings and model references. If credential import fails, retain the original ciphertext and offer re-entry without blocking project recovery.

Existing integrations remain on PatentForge until an explicit compatibility/cutover decision. HTTP contract parity alone does not prove compatibility with launchers that expect legacy executables and multiple processes.

Synthetic migration fixtures can demonstrate imported projects/history without touching private installations. Legacy history may lack original per-run prompt/provider snapshots; reconstructed import metadata must not be presented as proof of the original request, and incompatible imported runs cannot be silently resumed with new prompts or destinations.

## Platform and distribution evidence

| Target | Starting coverage and open evidence |
| --- | --- |
| Windows | Native build and acceptance required. Legacy installer bootstrap headers do not establish application CPU coverage. Select and verify the Rust target rather than inferring it from the old installer. |
| macOS | Preserve confirmed arm64 coverage. Legacy minimum-OS metadata conflicts; resolve support floors through native validation and an explicit support decision. Intel/universal expansion is not established by existing evidence. |
| Linux | Preserve confirmed x86_64 coverage. Validate actual OS/driver requirements and desktop-session behavior, rather than assuming AppImage creation proves compatibility. |

Pin Rust and GPUI before implementation builds. Derive OS and graphics requirements from that revision and actual results. Do not silently raise a promised support floor.

Build all three platforms from the start. Existing workflow runner labels are not evidence of available or authorized runners. Obtain native runner access and isolated interactive sessions before promising installed results, while independent work continues.

Package Full and Lean from one integrated revision into Windows Setup EXEs, macOS DMGs and Linux AppImages. Separate private artifact preparation from signing, tagging, upload and publication. Windows signing-policy ownership and macOS signing/notarization remain later decisions. Preserve any applicable maintainer review and signing approval requirements.

## Validation and legal constraints

Use synthetic/recorded adapters first, then real authorized local inference through the same command path. Verify deterministic inputs, policy, event ordering, persisted records and document structure, not exact live-model prose. Paid calls and research opt-ins must be enforced in the core for every entry point, including resume, rerun and API requests. No silent Cloud fallback.

Keep core tests, native builds, native interaction, live providers, packages and installed results separately identified. Headless browser tests do not validate GPUI or OS integration. Do not turn renderer selection into a framework trial or GPU benchmark project.

No source license is selected here. Review applicable licenses and notices before bringing in legacy material. Preserve prompt text/provenance, required attribution, prompt-license obligations, notices and disclaimers; source and prompt licensing must not be conflated.
