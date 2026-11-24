# Feasibility prompt provenance

The seven prompt files `common-rules.md` and `stage-1.md` through `stage-6.md`
are copied without text changes from
[Scott Converse's PatentForge](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623/services/feasibility/src/prompts),
commit `7a3200c8d861c2d291e07d8c8bc6c6e44417f623`.
Copyright (c) 2026 Scott Converse.

These prompts are licensed under
[Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/).
`LICENSE-PROMPTS` and `LEGAL_NOTICE.md` are retained verbatim here. All original
embedded disclaimers remain present, including original product names. No
endorsement is implied, and no application-wide relicensing is implied.

The Rust narrative/message composition and context retrieval are adaptations
of the upstream MIT-licensed feasibility runner/context code; applicable
source attribution is retained by the repository integration owner.
The system message remains common rules + two newlines + the stage template.
The aggregate snapshot is SHA-256 of length-prefixed UTF-8 file contents in
common/stage1..6 order; it is intentionally not an individual file checksum.

The first-slice intake preserves title, description and all nine original
optional invention fields in their original narrative labels/order, omitting
blank values and trimming nonblank values. Technical field is appended as a
separate additional label, not substituted for legacy context. Research is opt-in per run:
without an explicit opt-in no prior-art section is inserted; with one, the recorded
references are rendered under "USPTO Prior Art Results". Recorded responses
are newly authored synthetic acceptance fixtures, not copied model outputs.