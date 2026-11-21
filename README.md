# Patent Forge Lite

Patent Forge Lite is a new, separate native patent research and drafting application. It is not an in-place upgrade, rename of the existing repository, or replacement installation of PatentForge.

The intended architecture is a standalone Rust application core with a GPUI desktop on Windows, macOS and Linux. The installed product will not require the legacy React, Node/NestJS, Python or Go application services. Ollama remains an independently managed inference runtime; authorized Cloud and research providers remain external services.

## Current status

Implementation is active on a first complete feasibility slice. The workspace contains shared Rust domain, storage and workflow crates, a CLI, and a GPUI desktop with portable presentation/input tests. Desktop source covers project/intake editing (including nine optional invention-context fields), explicit execution setup, progress/cancel/resume, history, native report selection and Markdown/HTML export through the shared core. Source presence is not a native or installed acceptance result.

The private [scottconverse/patent-forge-lite repository](https://github.com/scottconverse/patent-forge-lite) is separate from PatentForge. No new application-wide source license has been selected.

Rust is pinned centrally to 1.98.1. GPUI and its platform adapter use the same official Zed revision. [Native UI notes](docs/native-ui.md) distinguish implemented source, headless evidence and still-required native/installed acceptance. Neither a dependency-only compile nor browser tests prove the native slice works.

## Delivery approach

1. Establish native Rust/GPUI builds for all three platforms from the start, with explicit runner access and platform evidence.
2. Deliver an early usable vertical slice: project and invention intake, a fixture-backed analysis, streaming, navigation away and back, interruption/recovery, saved report and export.
3. Replace recorded responses with real, explicitly authorized local inference through the same application entry point. Keep replay and live results separate.
4. Expand through research, claims, compliance, application drafting, data compatibility, authenticated API compatibility and full export/lifecycle coverage.
5. Prepare and validate Full and Lean packages on each platform. Signing and publication have separate authorization gates.

Full and Lean are later setup/distribution profiles of **one product**, sharing the same Rust core and GPUI interface. "Lite" is the product name, not permission to drop Full or existing functional scope. The first slice exposes explicitly selected synthetic replay and existing local inference; Cloud/paid providers are fail-closed. Later Lean setup will reach Cloud without acquiring Ollama; Full will support Local/Cloud setup and existing-model reuse.

Native usability comes early: text editing, selection, clipboard, focus, keyboard conventions and long-document navigation. Framework comparisons and a GPU benchmark project are outside scope.

## Coexistence with PatentForge

- Keep the existing PatentForge repository, installation, services, projects and models untouched.
- The desktop uses application ID `com.scottconverse.patentforgelite`, a dedicated per-user `patent-forge-lite` data directory and an OS-backed profile lock. Packaging/configuration identities must remain distinct as later distribution work lands.
- Do not claim existing ports, alter old launchers, stop independently owned Ollama/DSH processes or redirect current integrations automatically.
- Migration is a later, opt-in, copy-first operation using authorized consistent backups. Do not open production data as a bootstrap shortcut.
- Future deprecation or cutover requires an explicit decision. Shipping this application does not deprecate PatentForge.

## Testing and permissions

Start with synthetic inventions, synthetic migration fixtures and deterministic recorded responses. Do not commit real inventions, customer records, credentials, machine-specific configuration or private paths. Provider credentials alone do not authorize inference, research, spending or disclosure.

Real local inference follows replay acceptance and requires explicit authorization for the runtime, model, endpoint and test data. A remote Ollama endpoint is not on-machine inference. No silent Cloud fallback is permitted. Paid Cloud calls, private-data import, interactive desktop trials, signing and external publication retain their applicable approval gates.

Cross-platform builds, native interaction, package creation and installed acceptance are separate claims. Runner labels or successful cross-compilation do not prove installed behavior.

## Compatibility and legal obligations

The functional reference remains [PatentForge v0.5.0](https://github.com/scottconverse/patentforge/releases/tag/v0.5.0), with applicable fixes at [the pinned source baseline](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623). This is a behavior reference, not a directive to copy its source or installation.

Before importing any source, prompt or asset, review its applicable license, attribution and notice requirements, including the legacy `LICENSE`, `LICENSE-PROMPTS` and legal notices. Preserve required prompt text, provenance, attribution, license notices and product disclaimers. A Rust rewrite does not remove those obligations or grant permission to relicense material. Workflow prompt provenance and upstream notices are maintained with the integrated core, not replaced by a new blanket license.

## Documents

- [Architecture and boundaries](docs/ARCHITECTURE.md)
- [First-slice acceptance](docs/FIRST-SLICE-ACCEPTANCE.md)
- [Native desktop source, controls and acceptance limits](docs/native-ui.md)
