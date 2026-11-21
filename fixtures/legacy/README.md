# Synthetic legacy fixture

`representative.sql` was authored for Patent Forge Lite storage tests from the
field/relationship contracts at the pinned PatentForge baseline. It is not copied
source, a production database, a patent disclosure, provider output or a prompt.
All records, ciphertext placeholders and content are synthetic.

Tests create disposable SQLite files from this SQL. It covers two feasibility
versions, completed and interrupted work, unknown usage versus zero, edits,
claim dependencies, compliance references, application versions, search links,
patent caches/family JSON, prosecution, nullable usage ownership and settings.
Raw archive comparisons preserve every source field in addition to the current
Lite domain projection. The fixture marker is required by the importer.

Feasibility run/stage completion uses the actual baseline value `COMPLETE`
(backend feasibility service and its rerun tests), not an invented `COMPLETED`
value. A separate migration test also exercises the tolerated `COMPLETED` alias.
