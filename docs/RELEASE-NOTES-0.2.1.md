# PatentForge Lite 0.2.1

<!--
Versioned notes. The release workflow publishes `docs/RELEASE-NOTES-<version>.md` and
refuses to publish when it is missing, so this file is the notes for a 0.2.1 release.
Writing the file is not publishing it: the release, its tag and its signing are separate
decisions recorded in docs/ACCEPTANCE.md.
-->

A standalone patent research and drafting workspace, built from the audit-hardened
revision. This release is a candidate until it is published, signed and tagged.

## What you can do

- **Four analyses** over one invention — feasibility, claims drafting, compliance review and
  application drafting — with dependency enforcement and an immutable snapshot of everything
  each run consumed.
- **Three ways to run** — recorded replay, your own local Ollama model, or Anthropic Cloud
  with explicit per-run paid authorization. No silent fallback, ever.
- **Edit and version** — edit a released stage, save it as the next numbered revision, and
  compare any revision with the one before it.
- **Choose what compliance reviews** — point a review at a specific claims or application
  revision; the run records which revision it examined, and a package only carries findings
  that match the revisions it exports.
- **Export exact revisions** — Markdown, HTML, Word and CSV with a hash manifest naming the
  revision; the same revision exports byte-identically.
- **Regenerate from here onward** — re-run a chosen stage and everything after it, carrying
  the earlier stages unchanged. Suffix only; the interface never promises a single stage.
  A paid regeneration asks for approval again rather than reusing an earlier one.
- **Research with consent** — USPTO PatentsView behind a session-only key and a per-run
  opt-in, with the recorded references shown on the run and in the IDS.
- **Migration** — copy-first import from a legacy database with a preview of what would move,
  provenance warnings, re-runnability and credentials redacted at import.

## Downloads

| Platform | Asset |
| --- | --- |
| Windows 10/11 x64 | `patent-forge-lite-0.2.1-x64.exe` |
| macOS 13+ (Apple silicon) | built by the release workflow; unsigned |
| Linux x86_64 | built by the release workflow; unsigned |

Each asset ships with a `.sha256` beside it. The Windows installer installs per-user and
uninstalling never deletes your projects. Signing is applied by the release workflow when
Azure credentials are configured; the workflow withholds an unsigned Windows installer rather
than publishing one.

## Known limits

- The Cloud provider is exercised against a loopback stand-in; a live paid run has not been
  made from this build.
- PatentsView has not been queried live.
- macOS and Linux packages are built but their installed behaviour is verified on Windows
  only; `docs/PLATFORM-RUNBOOK.md` records the checks for those machines.
- The HTTP API's write endpoints are not verified.
