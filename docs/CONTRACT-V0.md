# Foundation contract v0

Canonical source: [`pfl-domain`](../crates/pfl-domain/src/lib.rs). Foundation
revision: `164670a5921032d145bc3ff2e05fe6aaceee8cda`.

## Data

- UUID-backed `ProjectId`, `RunId`, `AttemptId` serialize as strings.
- Projects save an editable `Intake`. A `NewRun` captures an immutable
  `RunInput`: intake, provider/model/destination, prompt version/hash and explicit
  authorization. There are no credential values in these contracts.
- Context delta `beba232b64dae60508dc9a941674d71cb920aef6` preserves the nine
  legacy optional invention fields in `Intake.context: InventionContext`.
  Missing/partial context defaults safely; camelCase legacy keys are accepted.
  `labeled_fields()` supplies the exact legacy narrative labels/order. All six
  provider stages receive the captured input, not later project edits.
- A per-project idempotency key identifies a start request; a changed request
  must not silently replace its original snapshot. Storage assigns the next
  project version atomically. Rerun creates another version; resume creates
  another attempt for the same run.
- `Stage::ALL`: technical restatement, prior art, patentability, landscape,
  IP strategy, final report. Stage numbers are one through six.
- `RunSnapshot` includes attempts, durable checkpoints, outcome, last event
  sequence and an optional saved report tied to the selected run/version.
- Outcomes distinguish prepared, running, cancelling, cancelled, failed,
  interrupted and completed. Usage `None` means unknown, never zero.

## Persistence and async boundary

`Store: Send + Sync` is synchronous. Every operation that may block is called
through `tokio::task::spawn_blocking` from async application orchestration.
Never hold a database mutex across an await. GPUI calls the async facade through
an explicitly owned Tokio runtime; closing a view does not own or abort jobs.

Store operations cover project list/create/load/intake/settings,
idempotent run creation, attempt acquisition, ordered events, atomic checkpoint
plus usage plus event, cancellation/terminal outcomes, saved history/report and
startup interruption recovery. Every attempt-owned write rejects stale attempts.
`recover_interrupted` is startup-only and must not be invoked by subscriptions,
project reopening or a second window.

`Provider` is an async trait with:

```rust,ignore
fn snapshot(&self) -> ProviderSnapshot;
async fn generate(
    &self,
    request: ProviderRequest,
    cancellation: CancellationToken,
    deltas: UnboundedSender<String>,
) -> Result<ProviderResponse>;
```

Streaming text is provisional until the runner persists its checkpoint.
Cancellation belongs to an attempt. A provider never silently changes
destination, calls Cloud, acquires a model or starts research.

## Commands and reads

`Command` distinguishes `Start(NewRun)`, `Resume(RunId)`,
`Rerun { source, request }`, and `Cancel(RunId)`.
`Subscribe { run_id, after_sequence }` is deliberately a separate read cursor.
Snapshot/history/report/export reads never start or resume work. Replay and
authorized local inference use the same runner and storage boundary.

Native desktop code is gated by the `native` feature. Portable desktop state
and input tests run without GPUI and do not establish native interaction.
The desktop lane owns the real native binary; the CLI is a headless integration
and synthetic-journey harness, not a substitute desktop or a framework trial.
