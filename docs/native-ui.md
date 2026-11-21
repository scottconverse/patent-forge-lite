# Native desktop

## Visual design

The interface is a layered dark theme built entirely from GPUI primitives — no
HTML, no CSS, no webview. The design language lives in
`crates/pfl-desktop/src/native/theme.rs`:

- **Surfaces**: four elevation levels (`BG` → `RAIL` → `SURFACE` → `SURFACE_2`) with
  1px borders and real box shadows (`BoxShadow` with blur and negative spread), so
  cards lift off the background instead of sitting flat.
- **Type scale**: `text_2xl` page titles, `text_lg` section titles, `text_sm` body,
  `text_xs` metadata, with `SEMIBOLD`/`MEDIUM` weights and muted colour tiers
  (`TEXT` / `MUTED` / `FAINT`).
- **Components**: an 84px icon rail with hand-drawn geometric icons (no icon font
  or SVG asset pipeline), segmented workflow tabs, status pills coloured by run
  outcome, a segmented checkpoint progress bar, label/value metadata rows, and
  primary/secondary/ghost button variants — each with hover state styling via
  GPUI's `hover()` helper.
- **Layout**: fixed top bar (identity + live mode + provider), scrolling content
  column, fixed status bar. Pages use flex rows/columns and wrap grids.

Keyboard reachability: `Ctrl+1`–`Ctrl+6` select Projects, Intake, Analysis, History,
Report and Setup; `Ctrl+S` saves; `Ctrl+Q` quits; `Tab`/`Shift+Tab` traverse fields.

## Launch options

```
pfl-desktop --replay             select synthetic recorded responses (starts nothing)
pfl-desktop --page analysis      open directly to projects|intake|analysis|history|report|setup
pfl-desktop --select-latest      open the most recent project and its latest run
pfl-desktop --help
```

`--select-latest` together with `--page report` is the fastest way to inspect real
seeded content without clicking.

## Implementation and evidence boundary

The desktop is a GPUI application, not a browser shell. Rust 1.98.1 and official GPUI/platform revision `2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4` are pinned centrally. The workspace retains Windows x86_64, macOS arm64 and Linux x86_64 targets. A target declaration is not a successful native build, available runner or installed result.

Desktop source currently provides:

- Project list/create/open, saved intake and all nine optional invention-context controls.
- A custom native text input using the portable Unicode/grapheme editor, UTF-16 IME handling, selection, clipboard, undo/redo, focus and native wrapped text.
- Explicit synthetic replay or existing-local-runtime configuration. No default or startup inference, runtime spawn, download, model pull or legacy scan.
- An application-lifetime async session around the shared SQLite store and `WorkflowService`. It serializes UI commands while workflow-owned jobs continue independently.
- Durable event replay, stage/status/stream display, cancellation, explicit matching-provider resume, saved reports and version history.
- A native save dialog passing the exact selected run to shared-core Markdown/HTML export. Existing destination files are rejected, never silently overwritten.

The real peers are now integrated into this crate. On Windows x86_64 with the pinned toolchain, `cargo fmt`, `cargo test -p pfl-desktop` (8 editor, 8 state, 7 session) and `cargo clippy -p pfl-desktop --features native --all-targets -- -D warnings` all pass, `cargo check -p pfl-desktop --features native` passes against the real GPUI revision, and `cargo build -p pfl-desktop --features native` produced a linked `pfl-desktop.exe`. That first real compile found one genuine GPUI API mismatch (`FocusHandle::focus` now requires `cx`), which is fixed. The session and its tests are no longer gated behind the native feature, so the actor and single-owner lock tests run headlessly.

No desktop window or GPU trial has been launched, and native keyboard/IME, dialog, rendering and installed behavior remain **not run**. A linked binary is not interactive acceptance; those checks still require an authorized isolated session.

## Safe entry point and execution

The native binary is `pfl-desktop`, built with the `native` feature. The default feature set is headless-testable and does not claim to be a desktop executable.

After an authorized native build, the intended launch command is `cargo run -p pfl-desktop --features native`. This opens a window and is **not** an instruction for an agent to bypass the interactive-launch gate. `--replay` explicitly configures synthetic recorded responses; it still does not create a project or start a run. `--help` prints usage without starting GPUI.

Normal startup opens only the dedicated Lite data profile, takes its OS-backed lock and performs startup-only interrupted-run recovery. The application ID is `com.scottconverse.patentforgelite`; the product directory is `patent-forge-lite`:

| OS | Data base |
| --- | --- |
| Windows | `LOCALAPPDATA` |
| macOS | `HOME/Library/Application Support` |
| Linux | `XDG_DATA_HOME`, otherwise `HOME/.local/share` |

The database is `projects.sqlite3`; its sibling lock is held for the process/session lifetime and released by the OS on death. A second owner cannot mark a live owner's runs interrupted. No legacy data/configuration, locks, credentials, ports or processes are reused.

Choose execution in **Execution setup**. Replay requires explicitly synthetic input. Local inference additionally requires a configured on-machine endpoint, an already-installed model and explicit authorization. Configuring never generates. Cloud and paid providers are fail-closed for this slice, with no fallback. Full/Lean are later distribution profiles of this one product, not implemented package claims.

## Editing, navigation and reports

1. Create a project, edit title, technical field and description, then expand optional context as needed.
2. The nine controls follow the canonical legacy labels/order. Collapsing them does not clear their values.
3. Save intake before explicitly starting an analysis. Switching views must not start, resume, cancel or duplicate execution.
4. Analysis shows durable checkpoints separately from transient streaming. Cancellation and resume go to the owned runner, not a view-owned task.
5. History selects a run/version. Report display and export use that exact identity, not "whichever run completed most recently."

The read-only report surface supports selection/copy and Page Up/Down/document-edge navigation. Editing controls support the platform's primary-modifier shortcuts for select/copy/cut/paste/undo/redo/save, plus Tab/Shift-Tab focus traversal. Actual OS clipboard, IME composition, focus geometry and wrapped-line behavior still need native validation.

Legacy synthetic import is a separate explicit harness operation, never an ordinary startup side effect. Reconstructed historical request metadata is not proof of original prompt inputs. Resume must retain compatible saved provider/model/prompt identity; an incompatible imported run needs an explicit new version instead.

This first slice closes the app when its last window closes and asks the core to shut down its own jobs. It does not implement a tray-resident background lifecycle yet. Quit/interruption must leave recoverable state and cannot terminate independently owned Ollama, PatentForge or DSH.

The quit guard counts every job this application still owns, in any project or historical run, not only the run the current view shows; navigation never cancels work, so leaving a running analysis behind a different view still warns. `ApplicationSession::active_jobs()` is that app-wide count, and it is covered by a headless test that starts a job, opens an unrelated project, and then asserts the count only drops once the run is terminal.

## Headless checks and interactive gate

Portable checks use `cargo test -p pfl-desktop --locked --offline`; the actor tests no longer require the `native` feature. The shared build owner schedules Cargo commands; workers must not compete for memory/disk. Actor tests exercise the real store/workflow with synthetic controlled replay, not a live provider.

The intended real-service checks cover safe startup/configuration, serialized/idempotent starts, reopen without generation, cancellation/resume without repeating durable checkpoints, selected older-version export during newer progress, exclusive profile ownership and child-process crash release. A passing mock input test is not OS-level IME evidence.

Before an indispensable interactive session, identify the exact integrated candidate, native target and isolated desktop. Request approval to exercise:

- First launch and project reopen without provider traffic.
- Every basic/optional field, Unicode/IME, mouse selection, keyboard traversal, clipboard, undo/redo and unsaved-edit safety.
- Synthetic replay, progress, navigation away/return, cancel/resume, saved-history selection and long-report scrolling/copy.
- Native file-dialog cancellation, explicit destination export, Unicode/version content and rejection of existing files.
- Explicit quit/restart and profile-lock recovery without touching other applications.

Real local inference follows synthetic acceptance and separately authorized endpoint/model/data use. macOS/Linux native runner/interactive evidence and all clean installed/package checks remain separate gates. No signing, tag, release, paid call, private migration or old-system cutover follows implicitly.
