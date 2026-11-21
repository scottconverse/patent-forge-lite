# Recorded provider

`synthetic-feasibility.json` was authored for Patent Forge Lite acceptance. It
contains six explicitly recorded synthetic responses, no real patent references,
private inventions, credentials, configuration, or data from an installed system.
It is not an upstream model recording and makes no claim of live inference or
research. The fixture exercises Unicode, streaming, persistence and report export.

The provider streams 32 Unicode scalar values per delta and reports usage as
unknown, not zero. Tests can change the per-delta delay to exercise cancellation.
Prompt provenance and required licensing are in `../../prompts/feasibility/`.
