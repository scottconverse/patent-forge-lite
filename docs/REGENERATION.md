# Targeted regeneration — semantics

Status: **semantics fixed; section-scoped implementation open.** The whole-run
`rerun` path exists and its invariants are tested. This note is the contract the
section-scoped version must satisfy.

## What regeneration is

A regeneration produces a **new run** whose input is derived from a completed
**base run**:

1. The new run copies the base run's `intake`, `upstream`, `prompts`, `provider`
   snapshot and `authorization` — it re-derives nothing from the project's
   current state, so a later claims or feasibility version cannot leak in.
2. The new run gets a fresh `RunId` and the project's next version number.
3. `rerun_of` records the base run id, so lineage is visible in history and in
   exports.
4. The base run is never mutated: its snapshot, checkpoints, events and saved
   report remain byte-identical.

## What the operator's edits do

Edits live in **revisions** (`DocumentRevision`), never inside runs. A
regeneration does not consume or alter revisions:

- the revision keeps its `base_run_id`;
- the new run does not adopt the revision's text implicitly — selection is always
  explicit (`start_workflow_with_selection`), so a regeneration cannot silently
  overwrite or absorb hand edits;
- nothing is deleted. Old revisions and old runs stay addressable by id.

## Section scope (the part still to build)

A section-scoped regeneration re-runs **the named stages of one workflow** rather
than the whole workflow:

- the new run's input is still the base run's snapshot;
- stages outside the selection are **copied from the base run's committed
  checkpoints**, marked as carried over rather than regenerated;
- the new run records which stages were regenerated, so a report can say so;
- if a selected stage fails or is truncated, the run terminates with that
  outcome and no partial stage is committed — the same rule as a normal run.

## Invariants (must be tested for the section version too)

| Invariant | Currently covered by |
| --- | --- |
| A rerun creates a new version | `rerun` tests |
| The base run is unchanged | `rerun` tests |
| Lineage is recorded (`rerun_of`) | `rerun` tests |
| A truncated or failed regeneration saves no partial stage | run-outcome tests |
| Revisions are never consumed or rewritten | revision tests |

## Not in scope

- Automatic regeneration on edit. Edits are saved explicitly.
- Regenerating across workflows (e.g. regenerating claims from an application
  run). Cross-workflow derivation goes through the normal prerequisites.
- Rewriting history. Old runs are immutable, always.
