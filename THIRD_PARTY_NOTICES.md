# Attribution and license boundaries

## PatentForge source and prompts

Reference: [Scott Converse's PatentForge](https://github.com/scottconverse/patentforge/tree/7a3200c8d861c2d291e07d8c8bc6c6e44417f623), pinned revision `7a3200c8d861c2d291e07d8c8bc6c6e44417f623`.

The original `LICENSE` (MIT, copyright 2026 Scott Converse),
`LICENSE-PROMPTS` (CC BY-SA 4.0, copyright 2026 Scott Converse), and
`LEGAL_NOTICE.md` are preserved verbatim. The old product name in that legal
notice is intentionally retained, not an application identity or installation
instruction. The same research-only, no-legal-advice limitations apply to
Patent Forge Lite and its generated output.

`LICENSE` preserves the upstream source grant; it does not silently select a
new application-wide license for independently written Patent Forge Lite code.
No Cargo package currently declares a new source license. Prompt licensing is
separate from source licensing and cannot be replaced with MIT.

The workflow prompt directory records exact source paths, hashes, retained
disclaimers and any changes. Original stage and common-rule text must not be
edited or stripped of attribution. Adaptations remain under CC BY-SA 4.0:
[license](https://creativecommons.org/licenses/by-sa/4.0/) and
[legal code](https://creativecommons.org/licenses/by-sa/4.0/legalcode).
Recorded responses and synthetic inventions are clearly labeled test material,
not real research, prior-art findings or live-provider evidence.

## GPUI

The native desktop uses the official
[zed-industries/zed source](https://github.com/zed-industries/zed/tree/2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4),
revision `2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4`.
`gpui` and `gpui_platform` declare Apache-2.0. They are dependency references,
not a copy or relicense of the complete Zed application. Its package-specific
and transitive license/notice obligations remain applicable. This file is not
a complete distribution SBOM or a claim that packaging license review passed.
