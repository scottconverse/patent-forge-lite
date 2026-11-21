# Application prompt provenance

The six prompt files `common-rules.md`, `background.md`, `summary.md`,
`detailed-description.md`, `abstract.md` and `figures.md` are copied without text
changes from
[Scott Converse's PatentForge](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623/services/application-generator/src/prompts),
commit `7a3200c8d861c2d291e07d8c8bc6c6e44417f623`.
Copyright (c) 2026 Scott Converse.

These prompts are licensed under
[Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/).
`LICENSE-PROMPTS` and `LEGAL_NOTICE.md` are retained verbatim here. All original
embedded disclaimers remain present, including original product names. No
endorsement is implied, and no application-wide relicensing is implied.

Source and per-file SHA-256 identity are recorded in the workflow test suite.
The system message is common rules + two newlines + the section template. The
aggregate snapshot is SHA-256 of length-prefixed UTF-8 file contents in
common/background/summary/detailed-description/abstract/figures order.

The upstream pipeline order is background → summary → detailed description →
abstract → figures, then a non-LLM IDS/format step and a markdown-artifact
cleanup. This implementation keeps that order and applies the same cleanup when
composing the saved report. The information disclosure statement table is built
from snapshot prior-art items without an LLM call, as upstream does.
