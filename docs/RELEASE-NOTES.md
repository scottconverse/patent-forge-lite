# Historical notes — v0.1.0 and earlier candidates

The release workflow publishes `docs/RELEASE-NOTES-<version>.md` and refuses to publish when it is
missing. The current release notes are `docs/RELEASE-NOTES-0.2.1.md`; this file is kept for the
history it records.

Built from revision `9e5d168`. Nothing here is published, tagged or signed; the artifact
and its SHA-256 are recorded in `docs/RELEASE-MANIFEST.md`. The release workflow publishes
`docs/RELEASE-NOTES-<version>.md` and refuses when it is missing, so this file is the draft
record rather than a fallback.

## New since 0.1.0

- **Editing that versions** — edit the selected stage's text in the window, save it as the
  next numbered revision anchored to the completed run, and list the series back.
- **Revision comparison** — every revision row offers a comparison against the previous
  one, rendered from the deterministic line diff.
- **Choose what compliance reviews** — pick a specific claims or application revision and
  start the compliance run against it; the chosen revision is recorded in the run's input.
- **Export exact revisions** — Markdown, HTML, Word and CSV with a hash manifest naming the
  revision; re-exporting the same revision produces identical bytes, measured across
  separate processes.
- **Regenerate from here onward** — re-run a chosen stage and everything after it, carrying
  the earlier stages byte-identically from the base run. Suffix only; the interface never
  promises a single stage.
- **Research with consent** — USPTO PatentsView behind a session-only key and a per-run
  opt-in, with the recorded references shown on the run. An opted-out run queries nothing.
- **Setup that asks** — first run asks Full or Lean before anything else; the model picker
  lists what the runtime actually reports and says plainly when it cannot ask.
- **Honest cost line** — the window states the model, that the total is unknown until
  usage arrives, and the rates that will apply.
- **Lifecycle** — a second launch is refused with exit code 3; quit closes when nothing is
  pending.
- **Migration** — copy-first import from a legacy database, with a preview of what would
  move, provenance warnings, preserved archive-only records, re-runnability, and
  credentials redacted so no secret crosses over.
- **Windows upgrade and rollback** — install 0.1.0, upgrade, roll back, with the workspace
  inventory unchanged at each step.

## Not verified in this candidate

- A live PatentsView query (no key).
- A migration of a real legacy database (needs authorization and a copy).
- Installed acceptance on macOS and Linux (no hardware); see `docs/PLATFORM-RUNBOOK.md`.
- HTTP write endpoints; only the read surface is verified (`docs/API-CONTRACTS.md`).
- Nav items are announced as tabs with a selectable action, measured in the window; the
  defect this file used to list here is closed.
# PatentForge Lite 0.1.0

The first public release: a standalone, native patent research and drafting workspace.

## What you can do

- **Four analyses** over one invention — feasibility (6 stages), claims drafting (4),
  compliance review (4) and application drafting (5) — with dependency enforcement and an
  immutable snapshot of everything each run consumed.
- **Three ways to run** — recorded replay (instant, offline, free), your own local Ollama model,
  or Anthropic Cloud with explicit per-run paid authorization. No silent fallback, ever.
- **Reports you can use** — rendered as documents, exportable to Markdown, HTML and real Word
  `.docx` from the same saved run identity.
- **Histories that survive** — every run is a durable version; interruption resumes from the last
  checkpoint instead of re-running and re-billing completed stages.
- **Keyboard-first** — `Ctrl+1`–`Ctrl+6` page navigation, visible focus everywhere, and a real
  accessibility tree (labelled controls, announced run status).

## Downloads

| Platform | Asset | Signing |
| --- | --- | --- |
| Windows 10/11 x64 | `patent-forge-lite-0.1.0-x64.exe` | **Signed** — Azure Trusted Signing (CN=Scott Converse) |
| macOS 13+ (Apple silicon) | `patent-forge-lite-v0.1.0-macos-arm64.zip` | Unsigned — right-click → Open on first launch |
| Linux x86_64 | `patent-forge-lite-v0.1.0-linux-x64.tar.gz` | Unsigned |

Each asset ships with a `.sha256` beside it. The Windows installer installs per-user (no
administrator rights) and uninstalling never deletes your projects.

## Verified for this release

- 104 Rust tests, `cargo fmt --check` and `clippy -D warnings` clean on Windows x64,
  macOS arm64 and Linux x86_64 in CI.
- Windows installer installed, launched, driven through 26 automated UI checks, then uninstalled
  with the project profile preserved.
- Local inference against Ollama: all four analyses, 19 stages, on a single model.
- Word export opened in Word without a repair prompt.

## Known limitations in this build

These are limitations of the shipped build, not future ideas:

- **Cloud is selectable with a paid approval per run.** The engine has an Anthropic adapter with a fail-closed paid-call
  gate, but the application offers only Recorded replay and Local. There is no key entry and no
  Cloud run path in the UI.
- **The Lean setup profile has no usable mode**, because it is Cloud-only and Cloud is not in the
  desktop yet.
- **`pfl-cli` is not included in the Windows installer.** Build it from source.
- **`pfl-cli serve` is replay-only.** It cannot reach a live provider.
- **Every analysis currently requires the synthetic-data confirmation.** There is no supported
  real-invention workflow in 0.1.0.
- **macOS and Linux are artifacts, not verified support.** They compile and pass their tests;
  nobody has installed or used them.

## Not verified, and not claimed

- No installed or interactive acceptance on macOS or Linux yet — those builds compile and pass
  tests, and that is all this release claims for them.
- Cloud inference is fail-closed and has only been verified against a mock, never a live endpoint.
- Live prior-art research is a seam with a fixture source; there is no external provider.

## Not legal advice

Every analysis is a model's opinion about the text you supplied. Models invent prior-art
references and misstate law. Verify everything, and have a registered patent attorney review
anything before filing.

## Requirements

Analysis needs an inference runtime unless you use recorded replay. Local mode expects an
already-installed [Ollama](https://ollama.com) and a model you have pulled; Lite never downloads
models or starts a runtime for you.

See the [user manual](https://github.com/scottconverse/patent-forge-lite/blob/main/docs/USER-MANUAL.md)
for the complete walkthrough, and [docs/STATUS.md](https://github.com/scottconverse/patent-forge-lite/blob/main/docs/STATUS.md)
for the honest inventory of what is and is not proven.
