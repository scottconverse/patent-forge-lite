# Claims prompt provenance

The four prompt files `common-rules.md`, `planner.md`, `writer.md` and
`examiner.md` are copied without text changes from
[Scott Converse's PatentForge](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623/services/claim-drafter/src/prompts),
commit `7a3200c8d861c2d291e07d8c8bc6c6e44417f623`.
Copyright (c) 2026 Scott Converse.

These prompts are licensed under
[Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/).
`LICENSE-PROMPTS` and `LEGAL_NOTICE.md` are retained verbatim here. All original
embedded disclaimers remain present, including original product names. No
endorsement is implied, and no application-wide relicensing is implied.

Source and per-file SHA-256 identity are recorded in the workflow test suite.
The system message is common rules + two newlines + the stage template, matching
the upstream agent loader. The aggregate snapshot is SHA-256 of length-prefixed
UTF-8 file contents in common/plan/draft/examine order.

The upstream pipeline is plan → draft → examine → conditional revise. This
implementation always executes a fourth revision pass and instructs it to keep
the claims unchanged when the examiner found nothing to fix, so that every run
has a contiguous, resumable checkpoint prefix. The examiner's own verdict is
still preserved verbatim in the checkpoint text.
