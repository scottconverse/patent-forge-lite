# Compliance prompt provenance

The five prompt files `common-rules.md`, `written-description.md`,
`definiteness.md`, `formalities.md` and `eligibility.md` are copied without text
changes from
[Scott Converse's PatentForge](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623/services/compliance-checker/src/prompts),
commit `7a3200c8d861c2d291e07d8c8bc6c6e44417f623`.
Copyright (c) 2026 Scott Converse.

These prompts are licensed under
[Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/).
`LICENSE-PROMPTS` and `LEGAL_NOTICE.md` are retained verbatim here. All original
embedded disclaimers remain present, including original product names. No
endorsement is implied, and no application-wide relicensing is implied.

Source and per-file SHA-256 identity are recorded in the workflow test suite.
The system message is common rules + two newlines + the check template. The
aggregate snapshot is SHA-256 of length-prefixed UTF-8 file contents in
common/written-description/definiteness/formalities/eligibility order.

The upstream pipeline order is written description → definiteness → formalities
→ eligibility, followed by a non-LLM aggregation step. This implementation keeps
that order and composes the aggregated report from the four checkpoints.
