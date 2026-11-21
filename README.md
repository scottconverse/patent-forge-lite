# Patent Forge Lite

Patent Forge Lite is a new, separate native patent research and drafting application. It is not an in-place upgrade, rename of the existing repository, or replacement installation of PatentForge.

The intended architecture is a standalone Rust application core with a GPUI desktop on Windows, macOS and Linux. The installed product will not require the legacy React, Node/NestJS, Python or Go application services. Ollama remains an independently managed inference runtime; authorized Cloud and research providers remain external services.

## Current status

The staged delivery approach and product identity have been approved. The Rust workspace
now contains the application core, not a stub: typed domain contracts, transactional SQLite
storage with a synthetic legacy importer, the six-stage feasibility workflow with recorded
replay and Ollama adapters, a GPUI desktop interface, and a `pfl-cli` harness that drives the
real core end to end.

On Windows x86_64 with the pinned `1.98.1` toolchain, the workspace formats, compiles, passes
`clippy -D warnings`, and passes its full test suite, and the native desktop feature compiles
against the pinned GPUI revision. `pfl-cli` demonstrates the complete deterministic journey:
import a synthetic legacy project, export its preserved edited report, run and recover an
interruption in a fresh process, and reopen the saved result without regenerating it.

Not yet established, and not claimed: a launched native window and interactive keyboard, IME,
GPU, dialog or installed behavior; live local inference or Cloud; macOS and Linux native
builds; packaging, signing and release. `docs/BUILD-EVIDENCE.md` records exactly which checks
ran, and `docs/FIRST-SLICE-ACCEPTANCE.md` keeps the unexercised items open.

The product remains the private [scottconverse/patent-forge-lite repository](https://github.com/scottconverse/patent-forge-lite), separate from PatentForge. No source license has been selected; upstream prompt and legal notices are preserved as described in `docs/BUILD-EVIDENCE.md` and `THIRD_PARTY_NOTICES.md`.

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
- Use a distinct application identity and separate writable data, configuration, log and installation locations. The application identity is `com.scottconverse.patentforgelite` with the per-user `patent-forge-lite` data namespace; no legacy location is opened.
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

- [Architecture and boundaries](docs/ARCHITECTURE.md)
- [First-slice acceptance](docs/FIRST-SLICE-ACCEPTANCE.md)
