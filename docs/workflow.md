# Workflow execution

## The four workflows

One run belongs to exactly one workflow. Stage numbering restarts at 1 inside each
workflow, and a checkpoint prefix must always be contiguous within that workflow.

| Workflow | Stages | Prompts |
| --- | --- | --- |
| Feasibility | technical restatement → prior art → patentability → landscape → IP strategy → final report | `prompts/feasibility` |
| Claims | strategy → draft → examiner review → revision | `prompts/claims` |
| Compliance | written description → definiteness → formalities → eligibility | `prompts/compliance` |
| Application | background → summary → detailed description → abstract → figures | `prompts/application` |

Dependencies are declared, not inferred: claims needs feasibility, compliance needs
feasibility and claims, application needs feasibility and claims. `start_workflow`
returns a clear error naming the missing prerequisite rather than substituting a
different run's output.

Each dependent run **snapshots** the upstream text it consumes into
`RunInput.upstream` at creation:
`IpStrategy`/`FinalReport` text for claims, the final claims text for compliance and
application, and the feasibility report as the specification. A later upstream
version never changes an existing run, and a resume never re-reads upstream.

Composing a report is per workflow. Feasibility and claims save the final stage's
text verbatim; compliance assembles its four findings; application assembles the
sections with an IDS table built from snapshotted prior-art items, without an LLM
call, matching the upstream generator's non-LLM formatting step.

## Application facade

`WorkflowService::new(Arc<dyn Store>, Arc<dyn Provider>, Handle) -> Arc<Self>`
owns async execution independently of GPUI views. The application owns the Tokio
runtime lifetime and calls `shutdown().await` before dropping it. Construction
does not contact providers or recover another instance's work.

Async facade methods:

- `create_project(Intake)`, `projects()`, `project(ProjectId)`,
  `save_intake(ProjectId, Intake)`, `settings()`, `save_settings(Settings)`.
- `start_project(ProjectId, String, Authorization)` snapshots current intake,
  provider and embedded prompts, then starts with the supplied idempotency key.
- `start(NewRun)`, `resume(RunId)`, `rerun(RunId, NewRun)`, `cancel(RunId)`,
  `execute(Command)`. Rerun creates a complete new version with a source link;
  stage-selective reruns remain later parity.
- Read-only `run(RunId)`, `history(ProjectId)`, `report(RunId)` and
  `events(Subscribe)`. Events are durable cursor polling, not a task-owning stream.
- `recover_interrupted()` only during app startup before commands; reads never
  infer that an active job belongs to a dead process. The platform single-instance
  owner must establish exclusive application ownership before recovery.
- `shutdown()` cancels and awaits owned attempts. Abrupt process exit leaves a
  recoverable active attempt; no independently managed runtime is terminated.
  Admission closes and the owned-job snapshot is collected under the same
  command gate used by all starts/resumes/reruns, so an in-flight registration
  cannot escape a completed shutdown.

All potentially blocking repository/index work uses `spawn_blocking`. No SQLite
lock or task-map guard is held over provider awaits. A command-serialization gate
prevents concurrent replacement attempts while cancellation drains; completion
does not acquire this gate. Provider generation is polled inside the owned runner,
not detached as a separate task, so cancellation or persistence failure drops the
request future before permitting replacement.

## Durability and original behavior

Inputs, exact provider destination/model, authorization and prompt hashes are
immutable per run. Resume rejects a different provider or prompt snapshot.
Completed stages form a contiguous prefix; resumed attempts reuse that prefix,
including interruption before stage 1 and after stage 6 but before finalization.
The same run ID/version remains selected. New idempotency keys are explicit new
runs, not an implicit effect of navigation or subscription.

The six original templates, common rules, narrative labels and stage message
structure preserve title/description and all nine original optional invention
fields. Each stage receives the immutable saved context, not current project
edits. Blank optional values are omitted and nonblank values trimmed in original
order; technical field is appended separately. Context retrieval uses
in-memory SQLite FTS5 with the original porter tokenizer, BM25 ordering, stage
query strings, 20-result cap, approximately 2,000 UTF-16-unit paragraph chunks,
full previous stage and excerpt substitution for earlier matched stages.
Resume rebuilds the index from saved outputs, avoiding a lost-index dependency.
No match retains full earlier output, as in the original runner's merge.

Checkpoint text, measured/unknown usage and checkpoint event commit together.
Streaming text is not a durable completed stage. Interrupted partial stages may
be regenerated; completed stages must not be repeated. Terminal persistence must
succeed before a saved report is returned. Provider failures produce a generic
safe failure notice; raw HTTP response bodies are not placed in durable errors.

## Providers and policy

`ReplayProvider::synthetic()` streams clearly labeled synthetic JSON fixtures,
never live requests. Usage is unknown (`None`), not an invented zero. Tests use
recorded delays and gated synthetic providers to cover cancellation races.

`OllamaProvider::new(endpoint, model)` validates only. It does not discover
installations, launch/stop runtimes, inspect secrets, pull models or make requests.
Actual calls use cancellable direct `/api/chat` NDJSON with the selected model,
no tools, no research calls, no redirects/proxies, and no alternate destination.
HTTP credentials/query/fragment in the endpoint are rejected. A remote endpoint
is explicitly remote even though the inference-provider kind is `Local`;
`is_on_machine()` distinguishes loopback. No model availability is fabricated.

Every entry path needs an explicit data declaration: synthetic test material or a
real-invention acknowledgement, never both. A real-invention run is local-only: Cloud
and prior-art research both refuse it, because either would send the invention off the
machine.

Local inference additionally requires explicit authorization for the immutable
endpoint/model. Cloud requires a paid approval **for that run**, carried in the run's own
authorization and checked by the service: a stored approval is not reused by resume or by
regeneration, both of which take a fresh one. Credential presence never authorizes a call,
and a run without the approval boolean is refused before any provider request.

Research is opt-in per run. Without the opt-in no query is made; with it the recorded
references are snapshotted into the run input and rendered under "USPTO Prior Art Results".
The query happens only after authorization and the idempotency check pass, so a refused
start sends nothing, and shutdown can cancel a query in flight.

NDJSON output preserves UTF-8 across network boundaries; real token counts come
from Ollama completion fields and remain unknown when absent. This slice's
domain has no monetary ledger; it must not present unknown costs as free.
Local generation uses the explicit first-slice 32,000 output-token cap; Cloud uses 16,000, which is above the largest prompt target. A richer
immutable inference-settings contract is later parity.

## Reports and boundaries

`export_markdown(&RunSnapshot)` and `export_html(&RunSnapshot)` validate the
selected report's run/version identity. Saved edits may legitimately differ from
the original generation checkpoint and are never overwritten to force equality.
Exports include the saved input,
provider/prompt identity and version rather than current edited project input.
Imported historical reports explicitly label input as reconstructed from intake
available at import, not recovered historical per-run input.
HTML is a self-contained, escaped plain-text report with UTF-8 and a restrictive
content security policy, not a rich Markdown renderer. The destination write is
owned by desktop/CLI. Word (`.docx`) is exported from the same saved run identity in both the report exporter and the package exporter, with identical bytes for the same revision.

Use only the new application's Store/paths. No old application service,
installation, private configuration, database or process is a runtime dependency.
Recorded tests prove orchestration, not native interaction, installed delivery,
real inference quality or live provider availability. No live call is performed
by construction, import, read, subscription, report or export.