# Patent Forge Lite

Patent Forge Lite is a new, separate native patent research and drafting application. It is not an in-place upgrade, rename of the existing repository, or replacement installation of PatentForge.

The intended architecture is a standalone Rust application core with a GPUI desktop on Windows, macOS and Linux. The installed product will not require the legacy React, Node/NestJS, Python or Go application services. Ollama remains an independently managed inference runtime; authorized Cloud and research providers remain external services.

## Current status

The staged delivery approach and product identity have been approved. This repository bootstrap contains documentation only. Coding scopes, shared contracts and toolchain versions have not yet been activated or selected. There is no runnable application, Cargo workspace, build result or installed acceptance result here.

The selected hosting destination is the private [scottconverse/patent-forge-lite repository](https://github.com/scottconverse/patent-forge-lite), separate from PatentForge. Repository creation and Git initialization are assigned separately; this bootstrap does not establish their completion. No source license has been selected.

## Delivery approach

1. Establish native Rust/GPUI builds for all three platforms from the start, with explicit runner access and platform evidence.
2. Deliver an early usable vertical slice: project and invention intake, a fixture-backed analysis, streaming, navigation away and back, interruption/recovery, saved report and export.
3. Replace recorded responses with real, explicitly authorized local inference through the same application entry point. Keep replay and live results separate.
4. Expand through research, claims, compliance, application drafting, data compatibility, authenticated API compatibility and full export/lifecycle coverage.
5. Prepare and validate Full and Lean packages on each platform. Signing and publication have separate authorization gates.

Full and Lean are planned setup/distribution profiles of **one product**, sharing the same Rust core and GPUI interface. "Lite" is the product name, not permission to drop the Full profile or existing functional scope. Lean reaches Cloud setup without acquiring Ollama; Full supports Local/Cloud setup and reuses an existing model where available.

Native usability comes early: text editing, selection, clipboard, focus, keyboard conventions and long-document navigation. Framework comparisons and a GPU benchmark project are outside scope.

## Coexistence with PatentForge

- Keep the existing PatentForge repository, installation, services, projects and models untouched.
- Use a distinct application identity and separate writable data, configuration, log and installation locations. Exact identifiers and paths are not selected yet.
- Do not claim existing ports, alter old launchers, stop independently owned Ollama/DSH processes or redirect current integrations automatically.
- Migration is a later, opt-in, copy-first operation using authorized consistent backups. Do not open production data as a bootstrap shortcut.
- Future deprecation or cutover requires an explicit decision. Shipping this application does not deprecate PatentForge.

## Testing and permissions

Start with synthetic inventions, synthetic migration fixtures and deterministic recorded responses. Do not commit real inventions, customer records, credentials, machine-specific configuration or private paths. Provider credentials alone do not authorize inference, research, spending or disclosure.

Real local inference follows replay acceptance and requires explicit authorization for the runtime, model, endpoint and test data. A remote Ollama endpoint is not on-machine inference. No silent Cloud fallback is permitted. Paid Cloud calls, private-data import, interactive desktop trials, signing and external publication retain their applicable approval gates.

Cross-platform builds, native interaction, package creation and installed acceptance are separate claims. Runner labels or successful cross-compilation do not prove installed behavior.

## Compatibility and legal obligations

The functional reference remains [PatentForge v0.5.0](https://github.com/scottconverse/patentforge/releases/tag/v0.5.0), with applicable fixes at [the pinned source baseline](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623). This is a behavior reference, not a directive to copy its source or installation.

Before importing any source, prompt or asset, review its applicable license, attribution and notice requirements, including the legacy `LICENSE`, `LICENSE-PROMPTS` and legal notices. Preserve required prompt text, provenance, attribution, license notices and product disclaimers. A Rust rewrite does not remove those obligations or grant permission to relicense material. No legacy source, prompts or assets are included in this bootstrap.

## Documents

- [Architecture and boundaries](docs/ARCHITECTURE.md)
- [First-slice acceptance](docs/FIRST-SLICE-ACCEPTANCE.md)
