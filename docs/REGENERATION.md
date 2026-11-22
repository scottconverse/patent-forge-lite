# Targeted regeneration — semantics

Status: **decided: suffix regeneration.** The feature is named **"regenerate from
here onward"** and re-runs the selected stage **and every stage after it**. The
section-scoped option is closed and must not be implemented. This note is the
contract for the suffix version.

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

## Suffix scope (the part still to build)

"Regenerate from here onward" re-runs the chosen stage **and every later stage of
that workflow**:

- the new run's input is still the base run's snapshot;
- stages **before** the chosen one are carried over from the base run's committed
  checkpoints, which is exactly the contiguous prefix the engine already requires;
- the new run records which stage it restarted from, so a report can say "regenerated
  from stage N onward";
- if a regenerated stage fails or is truncated, the run terminates with that
  outcome and no partial stage is committed — the same rule as a normal run.

This keeps the durability invariant intact: checkpoints remain a contiguous
completed prefix, so resume and recovery behave exactly as they do today.

## Invariants (must be tested for the suffix version too)

| Invariant | Currently covered by |
| --- | --- |
| A rerun creates a new version | `rerun` tests |
| The base run is unchanged | `rerun` tests |
| Lineage is recorded (`rerun_of`) | `rerun` tests |
| A truncated or failed regeneration saves no partial stage | run-outcome tests |
| Revisions are never consumed or rewritten | revision tests |
| Stages before the restart point are byte-identical to the base run | to be written with the implementation |

## Not in scope

- Automatic regeneration on edit. Edits are saved explicitly.
- Regenerating across workflows (e.g. regenerating claims from an application
  run). Cross-workflow derivation goes through the normal prerequisites.
- Rewriting history. Old runs are immutable, always.

## Implementation constraint found (2026-11-21)

The run engine requires committed checkpoints to form a **contiguous completed prefix** of the workflow's stages (service report path, and execution resumes from stages.skip(checkpoints.len())). This invariant is what makes resume safe, and a section regeneration that carries non-adjacent stages would violate it.

**Decision (Scott, 2026-11-21): option 2, suffix regeneration, renamed "regenerate from here onward".** The prefix invariant is kept, the durability guarantee is untouched, and the UI must not promise single-stage regeneration.

1. **Sparse checkpoints.** Add a marker to Checkpoint (serde-defaulted) saying whether it was generated or carried over, and relax the prefix rule to allow carried gaps. Faithful to section scope, but touches the durability invariant that every existing resume test depends on.
2. **Regenerate from the first selected stage onward.** Keep the prefix rule untouched: a selection behaves as 're-run this stage and everything after it', with earlier stages carried as a genuine prefix. Simple and safe, but the name must change (it is a suffix regeneration, not an arbitrary section).

No code was changed for this: choosing option 1 is a durability change and option 2 is a scope change, and both are product decisions rather than implementation details.

