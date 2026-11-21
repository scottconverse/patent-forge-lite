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
