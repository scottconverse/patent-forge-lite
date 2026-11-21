# Patent Forge Lite

Patent Forge Lite is a new, separate native patent research and drafting application. It is not an in-place upgrade, rename of the existing repository, or replacement installation of PatentForge.

The intended architecture is a standalone Rust application core with a GPUI desktop on Windows, macOS and Linux. The installed product will not require the legacy React, Node/NestJS, Python or Go application services. Ollama remains an independently managed inference runtime; authorized Cloud and research providers remain external services.

## Current status

The staged delivery approach and product identity have been approved. The Rust workspace
contains the application core: typed domain contracts, transactional SQLite storage with a
synthetic legacy importer, **four analysis workflows** (feasibility, claims drafting,
compliance review, application drafting — 19 stages with the upstream prompts ported
verbatim), recorded-replay, Ollama and Anthropic Cloud adapters, a compatibility HTTP API,
a GPUI desktop interface, and a `pfl-cli` harness that drives the real core end to end.

On Windows x86_64 with the pinned `1.98.1` toolchain, the workspace formats, compiles, passes
`clippy -D warnings`, and passes 96 tests, and the native desktop feature compiles and links
against the pinned GPUI revision. `pfl-cli pipeline` runs all four workflows in dependency
order on one project, exports Markdown, HTML and Word for each, and reopens the profile
without regenerating anything. `pfl-cli serve` exposes the legacy-compatible HTTP surface.

Not yet established, and not claimed: a launched native window and interactive keyboard, IME,
GPU, dialog or installed behavior; live local or Cloud inference; live research; macOS and
Linux native builds; packaging, signing and release. `docs/STATUS.md` is the honest inventory,
`docs/BUILD-EVIDENCE.md` records exactly which checks ran, and
`docs/FIRST-SLICE-ACCEPTANCE.md` keeps the unexercised items open.

The product remains the private [scottconverse/patent-forge-lite repository](https://github.com/scottconverse/patent-forge-lite), separate from PatentForge. No source license has been selected; upstream prompt and legal notices are preserved as described in `docs/BUILD-EVIDENCE.md` and `THIRD_PARTY_NOTICES.md`.

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
- The desktop uses application ID `com.scottconverse.patentforgelite`, a dedicated per-user `patent-forge-lite` data directory and an OS-backed profile lock that the CLI honours too; no legacy location is opened. Packaging/configuration identities must remain distinct as later distribution work lands.
- Do not claim existing ports, alter old launchers, stop independently owned Ollama/DSH processes or redirect current integrations automatically.
- Migration is a later, opt-in, copy-first operation using authorized consistent backups. Do not open production data as a bootstrap shortcut.
- Future deprecation or cutover requires an explicit decision. Shipping this application does not deprecate PatentForge.

## Testing and permissions

Start with synthetic inventions, synthetic migration fixtures and deterministic recorded responses. Do not commit real inventions, customer records, credentials, machine-specific configuration or private paths. Provider credentials alone do not authorize inference, research, spending or disclosure.

Real local inference follows replay acceptance and requires explicit authorization for the runtime, model, endpoint and test data. A remote Ollama endpoint is not on-machine inference. No silent Cloud fallback is permitted. Paid Cloud calls, private-data import, interactive desktop trials, signing and external publication retain their applicable approval gates.

Cross-platform builds, native interaction, package creation and installed acceptance are separate claims. Runner labels or successful cross-compilation do not prove installed behavior.

## Compatibility and legal obligations

The functional reference remains [PatentForge v0.5.0](https://github.com/scottconverse/patentforge/releases/tag/v0.5.0), with applicable fixes at [the pinned source baseline](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623). This is a behavior reference, not a directive to copy its source or installation.

Before importing any source, prompt or asset, review its applicable license, attribution and notice requirements, including the legacy `LICENSE`, `LICENSE-PROMPTS` and legal notices. Preserve required prompt text, provenance, attribution, license notices and product disclaimers. A Rust rewrite does not remove those obligations or grant permission to relicense material. The only legacy material imported so far is the seven feasibility prompt files, byte-identical to the pinned source and accompanied by `prompts/feasibility/LICENSE-PROMPTS`, `LEGAL_NOTICE.md` and `PROVENANCE.md`; no legacy application source or assets have been copied.

## Documents

- [Product status: what works and what does not](docs/STATUS.md)
- [Architecture and boundaries](docs/ARCHITECTURE.md)
- [Compatibility API](docs/API.md)
- [First-slice acceptance](docs/FIRST-SLICE-ACCEPTANCE.md)
- [Native desktop source, controls and acceptance limits](docs/native-ui.md)
- [Build and acceptance evidence](docs/BUILD-EVIDENCE.md)
- [Storage and migration](docs/storage-and-migration.md)
- [Workflow execution](docs/workflow.md)
