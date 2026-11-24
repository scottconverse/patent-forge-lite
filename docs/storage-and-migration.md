# Storage and synthetic migration

## Identity and public interfaces

`pfl_storage::SqliteStore::open(path)` opens only the explicitly selected Lite
database. It never discovers installation paths, imports settings, reads private
credentials, binds ports or starts providers. The platform owner selects Lite's
separate per-user data/config identities and single-instance scope. The storage
constructor does not run startup recovery.

The SQLite application ID is `0x50464c31`; schema version 1 is recorded in
`schema_migrations`. An existing non-Lite database is rejected, not initialized
or renamed. Use `SqliteStore::in_memory()` for disposable core tests. Both
constructors return `pfl_domain::Result<SqliteStore>`.

The canonical synchronous `pfl_domain::Store` trait is implemented exactly.
SQLite uses foreign keys, WAL, synchronous FULL, a busy timeout and immediate
transactions. A mutex serializes access to one connection; SQLite transactions
also serialize separate store instances/processes. Async callers must use their
blocking boundary and never hold a storage lock across `.await`.

Projects, intake revisions, settings, immutable run inputs, versions, attempts,
checkpoints, usage, saved reports and sequenced events are durable. Independent
IDs and relational uniqueness constrain project/run versions and idempotency
keys. Domain JSON preserves provider/model/prompt/authorization snapshots
without adding ad hoc shared-domain fields.

- `create_run`: same project/key/input/rerun source returns the original run;
  changed input with the same key conflicts. New keys allocate the next version.
- `begin_attempt`: an active or completed run conflicts; interrupted/failed/
  cancelled work can obtain a new attempt on the same run. Only explicit startup
  `recover_interrupted` converts abandoned active attempts to Interrupted.
- Stage checkpoint + usage + event commit atomically, in stage order. Exact
  retry in the same running attempt returns the original event. Changed stage
  output, mismatched owners and stale attempts conflict.
- Cancelling is an atomic state/event change. It rejects all subsequent stage,
  delta and checkpoint writes, including checkpoint retries. Terminal
  cancellation still uses `finish_attempt`, preserving prior checkpoints.
- Finishing with a saved report requires every checkpoint of **that workflow** — six for
  feasibility, four for claims or compliance, five for application — plus matching run/version
  identity. Report, terminal attempt state and terminal event commit together. Identical finish
- Subscriptions and saved-result reads never start, resume or recover work.
  Cursor sequences are durable; transient deltas are not completed checkpoints.

`intake_history(&ProjectId)` returns immutable `IntakeRevision` records.
`save_revision`, `revisions` and `revision` are the document-revision store: numbered per
project and kind, anchored to the run a revision was edited from, and listed in order. A
revision with no text is refused, and a stored revision is never mutated.
Monetary cost is not a current shared-domain field. Optional token usage retains
unknown as `None`, not zero; legacy monetary values are preserved in the archive.

## Synthetic fixture and import APIs

This first-slice importer is **synthetic-only**, not authorized production import.

```rust,ignore
use pfl_storage::{write_legacy_synthetic_fixture, migrate_legacy_synthetic, SqliteStore};

write_legacy_synthetic_fixture(&new_source_path)?; // refuses an existing file
let receipt = migrate_legacy_synthetic(&new_source_path, &empty_migration_directory)?;
let imported = SqliteStore::open(receipt.activated_path.as_ref().unwrap())?;
```

`LEGACY_SYNTHETIC_FIXTURE_SQL` exposes the authored fixture SQL also stored in
`fixtures/legacy/representative.sql`. Its required marker table is
`PFLFixture(key TEXT PRIMARY KEY, value TEXT)`, with `synthetic=true` and
`fixture_id=pfl-legacy-synthetic-v1`. No external installation is scanned.

The representative project ID is `synthetic-project`. Its run
`synthetic-run-v1` is a completed version 1 with six checkpoints and an edited
Unicode saved report, directly readable via normal Store run/history methods.
Run `synthetic-run-v2` is interrupted version 2 with stage 1 complete and no
active attempt. A new explicitly authorized run becomes version 3.

There are two import forms:

1. `migrate_legacy_synthetic(source: &Path, destination: &Path)` returns an
   `ImportReceipt` with a separately activated Lite database path.
2. `store.import_legacy_synthetic(source: &Path, workspace: &Path)` first makes
   the same verified backup, then imports transactionally into that explicitly
   opened Lite store. Its receipt has no standalone activation path.

Both return the same receipt identity/counts on identical repeated imports,
with `repeated=true`, without duplicating records. Changed records reusing an
identity conflict. Existing Lite settings require an explicit merge policy;
the importer refuses to overwrite them.

The receipt's serializable, backward-compatible `warnings` field states that
historical run inputs are reconstructed, original saved reports are not Lite
generation, and archive-only fields are not yet usable workflow parity.

## Safety and recovery boundaries

1. Open the explicit marked source read-only; pin a read transaction. Check
   SQLite integrity and declared foreign keys.
2. Use SQLite's backup API, not a filesystem copy of the live main file. This
   includes committed WAL records in a standalone backup. Compare the complete
   source/backup record snapshots and validate integrity.
3. Use a new or fixture-owned workspace. An ownership marker prevents unrelated
   file replacement. A SQLite exclusive migration lease serializes migration
   and is released by the OS after a crashed process; no stale PID is killed.
4. Transactionally import the complete field archive, current domain
   projections, events and an identity/snapshot ledger into staging.
5. Validate staging; checkpoint and close it. Atomically publish a same-directory
   hard link to the new `patent-forge-lite.sqlite3`, then remove the owned staging
   name. Hard-link creation cannot replace an existing target. Filesystems
   without hard-link support fail safely, retaining the source/backup/staging.

Backup, stage and activation boundaries have deterministic interruption hooks:
`migrate_legacy_synthetic_with_options` accepts
`MigrationOptions { interrupt_after: Some(MigrationPhase::Stage) }` (also
`Backup`, `Activation`). Retry uses validated backup/staging/ledger state.
An incomplete or mismatching pending backup is rejected, never overwritten.
The original is always retained. Restoring the legacy backup means copying that
standalone backup to a new explicit location, not editing the original.

Atomic activation is not a guarantee against hardware/filesystem power-loss
behavior. Physical power-cut, native installed upgrade and real user-data
recovery are not established by synthetic tests.

## Projection versus archive-only compatibility

Active projection covers projects/intake including all nine legacy optional
context fields and the separate technical field, feasibility versions, terminal or
interrupted status, completed checkpoint text/token usage, saved report identity
and content, setup profile and local endpoint/model references. Imported runs
are historical, not evidence of execution by Lite. They do not authorize a
provider; unknown historical prompts/provider routing are explicitly marked,
never invented as current execution settings. The legacy schema did not
snapshot per-run intake, so imported run inputs reflect the source intake
available at import, not a claim to reconstruct unavailable earlier edits.
Lite intake edits cannot mutate those imported snapshots or saved reports.

The `legacy_records(identity, table)` archive preserves every source field with one
deliberate exception: **credential values are redacted at import**. Every non-credential
column is kept verbatim — original intake fields, timestamps/status, raw partial stage text,
claims/dependencies, compliance links, edited application versions, prior-art links/results,
patent/family caches, prosecution records, nullable usage ownership, export paths and monetary
costs — while a credential-named column stores a redaction marker instead of its value.
`is_credential_column` matches credential suffixes (`apikey`, `token`, `secret`, `password`,
`salt`, `key`, `credentials`, `authheader`, `authorization`) and URL userinfo is stripped from
a legacy endpoint before it is copied. Credentials must be re-entered for the new session; the
archive is not a place to recover them.

Legacy credential derivation or decryption is not attempted: the values are redacted, not
carried. Key re-entry happens in the new application and is session-only. Rich typed workflow
migration parity for archive-only entities and a real-database import remain authorized work.
unrecognized textual timestamps remain exact in the archive and project as 0.
No DSH endpoint, legacy executable, old database or model is changed.

## Validation

Storage tests cover persistence/reopening, Unicode intake revisions, settings,
immutable run snapshots, concurrent/repeated start, one active attempt,
checkpoint/event and report/event rollback on injected SQLite failures, stale
attempts, cancellation races, interrupted/resumed work before stage 1 and later,
child-process abrupt exit with an uncommitted transaction, saved report/history,
cursor replay and functional bundled FTS5.

Migration tests cover complete source-field comparisons, active saved-report
projection, WAL-only committed content, backup restore, repeated import,
transaction rollback, boundary interruptions, changed-source rejection,
unmarked-source rejection, unowned-workspace preservation and old partial
schemas, receipt provenance and fixture-builder no-overwrite behavior. Fixtures
contain no real data or credentials.

The central integration owner runs Cargo and records exact results. At authoring
time these tests await that resource-controlled run; source presence alone is
not evidence they passed. Native builds/interaction on each OS, actual providers,
installed behavior and production migration are separate, unclaimed gates.