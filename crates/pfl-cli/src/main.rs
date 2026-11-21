use async_trait::async_trait;
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::{OllamaProvider, ReplayProvider, WorkflowService, export_html, export_markdown};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc::UnboundedSender;

const MARKER: &str = "patent-forge-lite.synthetic.v1";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SyntheticWorkspace {
    format: String,
    synthetic: bool,
    project_id: Option<ProjectId>,
    run_id: Option<RunId>,
    database: PathBuf,
}

struct ReplayHarness {
    inner: ReplayProvider,
    calls: AtomicUsize,
    pause_stage: Option<usize>,
    workspace: PathBuf,
}

#[async_trait]
impl Provider for ReplayHarness {
    fn snapshot(&self) -> ProviderSnapshot {
        self.inner.snapshot()
    }

    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.pause_stage == Some(request.stage.number()) {
            let path = self.workspace.join("fixture-paused.json");
            let body = serde_json::json!({
                "format": MARKER,
                "stage": request.stage.number(),
                "run_id": request.run_id,
            })
            .to_string();
            tokio::task::spawn_blocking(move || std::fs::write(path, body))
                .await
                .map_err(|e| Error::Io(e.to_string()))?
                .map_err(|e| Error::Io(e.to_string()))?;
            cancellation.cancelled().await;
            return Err(Error::Cancelled);
        }
        self.inner.generate(request, cancellation, deltas).await
    }
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

/// The same sibling OS lock the desktop takes. A synthetic marker does not make
/// concurrent recovery safe, so a CLI must refuse a profile another owner holds.
fn take_profile_lock(database: &Path) -> Result<std::fs::File> {
    let path = database.with_extension("lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::Io(e.to_string()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|e| Error::Io(e.to_string()))?;
    file.try_lock().map_err(|_| {
        Error::Conflict("Patent Forge Lite is already using this data location".into())
    })?;
    Ok(file)
}

fn write_marker(path: &Path, marker: &SyntheticWorkspace) -> Result<()> {
    let body = serde_json::to_vec_pretty(marker).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::write(path.join("synthetic-workspace.json"), body)
        .map_err(|e| Error::Io(e.to_string()))
}

fn read_marker(path: &Path) -> Result<SyntheticWorkspace> {
    let bytes = std::fs::read(path.join("synthetic-workspace.json"))
        .map_err(|_| invalid("workspace has no synthetic fixture marker"))?;
    let marker: SyntheticWorkspace = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("invalid synthetic workspace marker"))?;
    if marker.format != MARKER || !marker.synthetic {
        return Err(invalid("refusing to open a non-synthetic workspace"));
    }
    if marker.database.as_os_str().is_empty()
        || marker
            .database
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(invalid(
            "synthetic database must remain inside its workspace",
        ));
    }
    Ok(marker)
}

fn create_workspace(path: &Path) -> Result<SyntheticWorkspace> {
    if path.exists()
        && std::fs::read_dir(path)
            .map_err(|e| Error::Io(e.to_string()))?
            .next()
            .is_some()
    {
        return Err(invalid(
            "new replay requires a new or empty workspace; use resume for an existing marked fixture",
        ));
    }
    std::fs::create_dir_all(path).map_err(|e| Error::Io(e.to_string()))?;
    let marker = SyntheticWorkspace {
        format: MARKER.into(),
        synthetic: true,
        project_id: None,
        run_id: None,
        database: PathBuf::from("patent-forge-lite.sqlite"),
    };
    write_marker(path, &marker)?;
    Ok(marker)
}

fn synthetic_intake() -> Intake {
    Intake {
        title: "SYNTHETIC FIXTURE: café sensor α".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
            "A fictional reusable cup contains a temperature sensor and an indicator. ",
            "The sensor is sampled by a controller that lights a blue indicator below a fixed threshold. ",
            "A second indicator reports an unavailable sensor rather than inventing a value. ",
            "A removable fixture supplies deterministic synthetic readings for restart tests. ",
            "This deliberately ordinary example exercises Unicode, storage and report export, ",
            "not novelty, commercial value, real prior-art search or legal advice. ",
        ].concat(),
        technical_field: "Synthetic sensor fixture".into(),
        context: InventionContext {
            problem_solved: " Synthetic problem α ".into(),
            how_it_works: " A fictional controller samples a fixture sensor. ".into(),
            ai_components: " No AI component in this synthetic example. ".into(),
            three_d_print_components: " A fictional removable enclosure. ".into(),
            what_is_novel: " No novelty assertion is made by this fixture. ".into(),
            current_alternatives: " Ordinary temperature indicators. ".into(),
            what_is_built: " Recorded test data only, no physical prototype. ".into(),
            what_to_protect: " Nothing, this is an integration fixture. ".into(),
            additional_notes: " Synthetic Unicode note: café 🧪 ".into(),
        },
    }
}

async fn wait_terminal(service: &WorkflowService, id: &RunId) -> Result<RunSnapshot> {
    wait_terminal_for(service, id, Duration::from_secs(60)).await
}

async fn wait_terminal_for(
    service: &WorkflowService,
    id: &RunId,
    limit: Duration,
) -> Result<RunSnapshot> {
    tokio::time::timeout(limit, async {
        loop {
            let snapshot = service.run(id.clone()).await?;
            if snapshot.outcome.is_terminal() {
                return Ok(snapshot);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| Error::Provider("run timed out".into()))?
}

async fn journey(command: &str, workspace: PathBuf, pause_stage: Option<usize>) -> Result<()> {
    let existing = command == "resume";
    let mut marker = if existing {
        read_marker(&workspace)?
    } else {
        create_workspace(&workspace)?
    };
    if command == "import-replay" {
        let source = workspace.join("legacy-synthetic.sqlite");
        let destination = workspace.join("imported");
        let receipt = tokio::task::spawn_blocking(move || {
            pfl_storage::write_legacy_synthetic_fixture(&source)?;
            pfl_storage::migrate_legacy_synthetic(&source, &destination)
        })
        .await
        .map_err(|e| Error::Storage(e.to_string()))??;
        let activated = receipt
            .activated_path
            .ok_or_else(|| invalid("synthetic import did not activate"))?;
        marker.database = activated
            .strip_prefix(&workspace)
            .map_err(|_| invalid("synthetic import activated outside its workspace"))?
            .to_path_buf();
        marker.project_id = Some(ProjectId("synthetic-project".into()));
        write_marker(&workspace, &marker)?;
    }
    let database = workspace.join(&marker.database);
    // Before any open or recovery, and held for the whole command.
    let _profile_lock = take_profile_lock(&database)?;
    let open_path = database.clone();
    let store = Arc::new(
        tokio::task::spawn_blocking(move || SqliteStore::open(open_path))
            .await
            .map_err(|e| Error::Storage(e.to_string()))??,
    );
    let preserved_report = if command == "import-replay" {
        let imported = store.run(&RunId("synthetic-run-v1".into()))?;
        let report = imported
            .report
            .as_ref()
            .cloned()
            .ok_or_else(|| invalid("import has no active saved report"))?;
        if !report.markdown.contains("Edited café report") || imported.checkpoints.len() != 6 {
            return Err(invalid(
                "import lost edited report, Unicode or checkpoint projection",
            ));
        }
        let prior_markdown = export_markdown(&imported)?;
        let prior_html = export_html(&imported)?;
        if !prior_markdown.contains("Edited café report")
            || !prior_html.contains("Edited café report")
        {
            return Err(invalid(
                "saved edited report could not be exported before new analysis",
            ));
        }
        std::fs::write(workspace.join("imported-v1.md"), prior_markdown)
            .map_err(|e| Error::Io(e.to_string()))?;
        std::fs::write(workspace.join("imported-v1.html"), prior_html)
            .map_err(|e| Error::Io(e.to_string()))?;
        Some(report)
    } else {
        None
    };
    let provider = Arc::new(ReplayHarness {
        inner: ReplayProvider::synthetic(),
        calls: AtomicUsize::new(0),
        pause_stage,
        workspace: workspace.clone(),
    });
    let service = WorkflowService::new(
        store.clone(),
        provider.clone(),
        tokio::runtime::Handle::current(),
    );
    let (id, expected_calls) = if existing {
        let id = marker
            .run_id
            .clone()
            .ok_or_else(|| invalid("fixture has no saved run identity"))?;
        let before = service.run(id.clone()).await?;
        if before.input.provider.kind != ProviderKind::Replay
            || !before.input.authorization.synthetic_data
            || Some(before.project_id.clone()) != marker.project_id
        {
            return Err(invalid("saved run does not match marked replay workspace"));
        }
        let remaining = Stage::ALL.len().saturating_sub(before.checkpoints.len());
        service.recover_interrupted().await?;
        service.resume(id.clone()).await?;
        (id, remaining)
    } else {
        let project = if command == "import-replay" {
            let project = service
                .project(ProjectId("synthetic-project".into()))
                .await?;
            // The imported historical description is deliberately below the explicit
            // minimum. Edit only the CURRENT intake, preserving every other imported
            // field, and never touch the historical snapshots or the edited report.
            let mut intake = project.intake.clone();
            intake.description = [
                "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
                "This imported fixture description is edited only so a new analysis version ",
                "can meet the explicit word minimum. Every other imported field is kept as-is, ",
                "the historical run inputs remain untouched, and the originally saved edited ",
                "report is preserved rather than regenerated. A fictional reusable cup still ",
                "contains a temperature sensor and an indicator driven by a controller that ",
                "lights a blue lamp below a fixed threshold and reports an unavailable sensor ",
                "instead of inventing a reading. No novelty, prior art, commercial or legal ",
                "conclusion is implied by any of this synthetic text.",
            ]
            .concat();
            service.save_intake(project.id.clone(), intake).await?
        } else {
            service.create_project(synthetic_intake()).await?
        };
        marker.project_id = Some(project.id.clone());
        write_marker(&workspace, &marker)?;
        let run = service
            .start_project(
                project.id,
                "synthetic-replay-v1".into(),
                Authorization {
                    synthetic_data: true,
                    ..Authorization::default()
                },
            )
            .await?;
        marker.run_id = Some(run.id.clone());
        write_marker(&workspace, &marker)?;
        (run.id, Stage::ALL.len())
    };
    let completed = wait_terminal(&service, &id).await?;
    if completed.outcome != RunOutcome::Completed
        || completed
            .checkpoints
            .iter()
            .map(|c| c.stage)
            .collect::<Vec<_>>()
            != Stage::ALL
    {
        return Err(Error::Conflict(
            "fixture did not complete exactly six ordered checkpoints".into(),
        ));
    }
    if provider.calls.load(Ordering::SeqCst) != expected_calls {
        return Err(Error::Conflict(
            "resume repeated completed provider stages".into(),
        ));
    }
    if provider.inner.request_count() != expected_calls
        || provider
            .inner
            .stage_calls()
            .values()
            .any(|count| *count != 1)
    {
        return Err(Error::Conflict(
            "actual replay invocations repeat or omit a required stage".into(),
        ));
    }
    for request in provider.inner.requests() {
        if request.input != completed.input
            || completed
                .input
                .intake
                .context
                .labeled_fields()
                .into_iter()
                .filter(|(_, value)| !value.trim().is_empty())
                .any(|(label, value)| {
                    !request.user_prompt.contains(label)
                        || !request.user_prompt.contains(value.trim())
                })
        {
            return Err(Error::Conflict(
                "stage request lost immutable invention context".into(),
            ));
        }
    }
    let calls_before_reads = provider.inner.request_count();
    let events = service
        .events(Subscribe {
            run_id: id.clone(),
            after_sequence: 0,
        })
        .await?;
    if events
        .iter()
        .enumerate()
        .any(|(i, e)| e.sequence != i as u64 + 1)
        || events.last().map(|e| e.sequence) != Some(completed.last_sequence)
    {
        return Err(Error::Conflict(
            "event sequence is not complete and consecutive".into(),
        ));
    }
    let markdown = export_markdown(&completed)?;
    let html = export_html(&completed)?;
    std::fs::write(workspace.join("report.md"), &markdown).map_err(|e| Error::Io(e.to_string()))?;
    std::fs::write(workspace.join("report.html"), &html).map_err(|e| Error::Io(e.to_string()))?;
    if std::fs::read_to_string(workspace.join("report.md")).map_err(|e| Error::Io(e.to_string()))?
        != markdown
        || std::fs::read_to_string(workspace.join("report.html"))
            .map_err(|e| Error::Io(e.to_string()))?
            != html
    {
        return Err(Error::Io("export roundtrip changed content".into()));
    }
    service.shutdown().await?;
    if provider.inner.request_count() != calls_before_reads {
        return Err(Error::Conflict(
            "read or export unexpectedly generated provider work".into(),
        ));
    }
    let reopened = tokio::task::spawn_blocking(move || SqliteStore::open(database))
        .await
        .map_err(|e| Error::Storage(e.to_string()))??;
    let saved = reopened.run(&id)?;
    if saved != completed
        || saved.report.is_none()
        || (!saved.input.intake.title.contains("café") && command != "import-replay")
    {
        return Err(Error::Conflict(
            "saved report or Unicode intake changed on reopen".into(),
        ));
    }
    if let Some(original) = &preserved_report {
        let prior = reopened.run(&RunId("synthetic-run-v1".into()))?;
        if prior.report.as_ref() != Some(original)
            || saved.version != 3
            || reopened.history(&saved.project_id)?.len() != 3
        {
            return Err(Error::Conflict(
                "new analysis changed imported saved history or version numbering".into(),
            ));
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "mode": "synthetic_recorded_replay", "outcome": "completed",
            "run_id": id, "version": saved.version, "attempts": saved.attempts.len(),
            "checkpoints": saved.checkpoints.len(), "events": events.len(),
            "provider_calls_this_process": provider.calls.load(Ordering::SeqCst),
            "exports": ["markdown", "html"], "reopened_without_generation": true,
            "imported_edited_report_preserved": preserved_report.is_some(),
        })
    );
    Ok(())
}

/// Counts real provider invocations without changing the adapter's identity, so
/// both replay and live journeys report actual calls rather than inferred ones.
struct CountingProvider {
    inner: Arc<dyn Provider>,
    calls: AtomicUsize,
}

impl CountingProvider {
    fn new(inner: Arc<dyn Provider>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            calls: AtomicUsize::new(0),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Provider for CountingProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        self.inner.snapshot()
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.generate(request, cancellation, deltas).await
    }
}

/// Runs every workflow in dependency order on one synthetic project, then
/// reopens the profile and proves nothing regenerated. Recorded responses only.
async fn pipeline(workspace: PathBuf) -> Result<()> {
    journeys(
        workspace,
        Arc::new(ReplayProvider::all_workflows()),
        "synthetic_recorded_replay",
        Duration::from_secs(120),
        None,
    )
    .await
}

/// The shared four-workflow journey. `live` passes a real provider here; the
/// only differences are the provider, the timeout and the recorded mode label.
async fn journeys(
    workspace: PathBuf,
    provider: Arc<dyn Provider>,
    mode: &str,
    run_timeout: Duration,
    local_setup: Option<(String, String)>,
) -> Result<()> {
    let mut marker = create_workspace(&workspace)?;
    let database = workspace.join(&marker.database);
    let _profile_lock = take_profile_lock(&database)?;
    let open = database.clone();
    let store = Arc::new(
        tokio::task::spawn_blocking(move || SqliteStore::open(open))
            .await
            .map_err(|e| Error::Storage(e.to_string()))??,
    );
    let counter = CountingProvider::new(provider);
    let service = WorkflowService::new(
        store.clone(),
        counter.clone(),
        tokio::runtime::Handle::current(),
    );
    if let Some((endpoint, model)) = &local_setup {
        // Selecting a local runtime is a Full-setup choice; Lean is Cloud-only.
        let mut settings = service.settings().await?;
        settings.profile = SetupProfile::Full;
        settings.local_endpoint = Some(endpoint.clone());
        settings.local_model = Some(model.clone());
        service.save_settings(settings).await?;
    }
    let project = service.create_project(synthetic_intake()).await?;
    marker.project_id = Some(project.id.clone());
    write_marker(&workspace, &marker)?;
    let mut runs = Vec::new();
    for kind in WorkflowKind::ALL {
        let run = service
            .start_workflow(
                project.id.clone(),
                kind,
                format!("pipeline-{}", kind.slug()),
                Authorization {
                    synthetic_data: true,
                    local_inference: local_setup.is_some(),
                    ..Authorization::default()
                },
            )
            .await?;
        let completed = wait_terminal_for(&service, &run.id, run_timeout).await?;
        if completed.outcome != RunOutcome::Completed
            || completed
                .checkpoints
                .iter()
                .map(|c| c.stage)
                .collect::<Vec<_>>()
                != kind.stages()
        {
            return Err(Error::Conflict(format!(
                "{} did not complete its exact ordered stages",
                kind.slug()
            )));
        }
        let report = completed
            .report
            .clone()
            .ok_or_else(|| invalid("completed workflow has no saved report"))?;
        let markdown = export_markdown(&completed)?;
        let html = export_html(&completed)?;
        std::fs::write(
            workspace.join(format!("{}-report.md", kind.slug())),
            &markdown,
        )
        .map_err(|e| Error::Io(e.to_string()))?;
        std::fs::write(
            workspace.join(format!("{}-report.html", kind.slug())),
            &html,
        )
        .map_err(|e| Error::Io(e.to_string()))?;
        let word = pfl_workflow::export_docx(&completed)?;
        std::fs::write(
            workspace.join(format!("{}-report.docx", kind.slug())),
            &word,
        )
        .map_err(|e| Error::Io(e.to_string()))?;
        runs.push(serde_json::json!({
            "workflow": kind.slug(),
            "run_id": completed.id,
            "version": completed.version,
            "stages": kind.stage_count(),
            "checkpoints": completed.checkpoints.len(),
            "report_bytes": report.markdown.len(),
        }));
    }
    let calls = counter.calls();
    service.shutdown().await?;
    // Reopen the profile: every workflow's saved result must survive unchanged
    // and no provider work may happen on read.
    let reopen = database.clone();
    let reopened = tokio::task::spawn_blocking(move || SqliteStore::open(reopen))
        .await
        .map_err(|e| Error::Storage(e.to_string()))??;
    let history = reopened.history(&project.id)?;
    if history.len() != WorkflowKind::ALL.len() || history.iter().any(|r| r.report.is_none()) {
        return Err(Error::Conflict(
            "reopen lost a workflow result or its saved report".into(),
        ));
    }
    if counter.calls() != calls {
        return Err(Error::Conflict(
            "reading the profile generated provider work".into(),
        ));
    }
    println!(
        "{}",
        serde_json::json!({
            "mode": mode,
            "outcome": "completed",
            "workflows": runs,
            "total_stages": 19,
            "provider_calls_this_process": calls,
            "history_versions": history.len(),
            "reopened_without_generation": true,
        })
    );
    Ok(())
}

/// Serves the compatibility API over one synthetic workspace. The token is read
/// from `PFL_API_TOKEN`; without it the API is open, matching legacy behaviour.
async fn serve(workspace: PathBuf, addr: String) -> Result<()> {
    let marker = read_marker(&workspace)?;
    let database = workspace.join(&marker.database);
    let _profile_lock = take_profile_lock(&database)?;
    let open = database.clone();
    let store: Arc<dyn Store> = Arc::new(
        tokio::task::spawn_blocking(move || SqliteStore::open(open))
            .await
            .map_err(|e| Error::Storage(e.to_string()))??,
    );
    let provider: Arc<dyn Provider> = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    service.recover_interrupted().await?;
    let token = std::env::var("PFL_API_TOKEN")
        .ok()
        .filter(|v| !v.is_empty());
    let state = pfl_api::ApiState::new(service.clone(), token.clone());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| Error::Io(format!("cannot bind {addr}: {e}")))?;
    let bound = listener
        .local_addr()
        .map_err(|e| Error::Io(e.to_string()))?
        .to_string();
    println!(
        "{}",
        serde_json::json!({
            "mode": "synthetic_recorded_replay",
            "listening": bound,
            "authentication": if token.is_some() { "bearer" } else { "open" },
        })
    );
    let served = pfl_api::serve_listener(listener, state)
        .await
        .map_err(|e| Error::Io(e.to_string()));
    service.shutdown().await?;
    served
}

/// Runs every workflow against an explicitly selected on-machine Ollama
/// endpoint/model, with synthetic input only. This is the only command that
/// performs real inference; it never downloads a model and never falls back.
async fn live(workspace: PathBuf, endpoint: String, model: String) -> Result<()> {
    let provider = Arc::new(OllamaProvider::new(&endpoint, &model)?);
    let on_machine = provider.is_on_machine();
    let started = std::time::Instant::now();
    journeys(
        workspace,
        provider,
        "live_local_inference",
        Duration::from_secs(3600),
        Some((endpoint.clone(), model.clone())),
    )
    .await?;
    eprintln!(
        "live summary: endpoint={endpoint} model={model} on_machine={on_machine} elapsed_seconds={:.1}",
        started.elapsed().as_secs_f64()
    );
    Ok(())
}

/// Read-only inventory of one profile database, using the same store the desktop
/// opens. Never creates a project, starts a run or touches another profile.
async fn inspect(database: PathBuf) -> Result<()> {
    let path = database.clone();
    let store = tokio::task::spawn_blocking(move || SqliteStore::open(path))
        .await
        .map_err(|e| Error::Storage(e.to_string()))??;
    let projects = store.list_projects()?;
    let mut summary = Vec::new();
    for project in &projects {
        let history = store.history(&project.id)?;
        let runs: Vec<serde_json::Value> = history
            .iter()
            .map(|run| {
                serde_json::json!({
                    "run_id": run.id,
                    "workflow": run.input.kind.slug(),
                    "version": run.version,
                    "outcome": run.outcome,
                    "checkpoints": run.checkpoints.len(),
                    "expected_stages": run.input.kind.stage_count(),
                    "report_chars": run.report.as_ref().map(|r| r.markdown.chars().count()),
                    "provider": run.input.provider.kind,
                    "model": run.input.provider.model,
                })
            })
            .collect();
        summary.push(serde_json::json!({
            "project_id": project.id,
            "title": project.intake.title,
            "runs": runs,
        }));
    }
    println!(
        "{}",
        serde_json::json!({
            "database": database,
            "projects": summary,
            "project_count": projects.len(),
        })
    );
    Ok(())
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = async {
        let command = args.first().map(String::as_str).ok_or_else(|| invalid(
            "usage: pfl-cli <replay|import-replay|pause-fixture|resume|pipeline|serve|live|inspect> --workspace <directory> [--stage <1..6>] [--addr <host:port>] [--endpoint <url> --model <name>] [--database <path>]"
        ))?;
        if !matches!(command, "replay" | "import-replay" | "pause-fixture" | "resume" | "pipeline" | "serve" | "live" | "inspect") {
            return Err(invalid("unknown command; only live performs real inference"));
        }
        let mut workspace = None;
        let mut stage = None;
        let mut addr = None;
        let mut endpoint = None;
        let mut model = None;
        let mut database = None;
        let mut i = 1;
        while i < args.len() {
            match (args[i].as_str(), args.get(i + 1)) {
                ("--workspace", Some(value)) if workspace.is_none() => workspace = Some(PathBuf::from(value)),
                ("--database", Some(value)) if database.is_none() => database = Some(PathBuf::from(value)),
                ("--addr", Some(value)) if addr.is_none() => addr = Some(value.to_string()),
                ("--endpoint", Some(value)) if endpoint.is_none() => endpoint = Some(value.to_string()),
                ("--model", Some(value)) if model.is_none() => model = Some(value.to_string()),
                ("--stage", Some(value)) if stage.is_none() => {
                    stage = Some(value.parse::<usize>().map_err(|_| invalid("invalid pause stage"))?);
                }
                _ => return Err(invalid("unknown, repeated or incomplete argument")),
            }
            i += 2;
        }
        if stage.is_some_and(|n| !(1..=6).contains(&n))
            || (command == "pause-fixture") != stage.is_some()
        {
            return Err(invalid("only pause-fixture accepts and requires --stage 1..6"));
        }
        if addr.is_some() != (command == "serve") {
            return Err(invalid("only serve accepts --addr"));
        }
        if (endpoint.is_some() || model.is_some()) && command != "live" {
            return Err(invalid("only live accepts --endpoint and --model"));
        }
        if command == "inspect" {
            return inspect(
                database.ok_or_else(|| invalid("inspect requires --database"))?,
            )
            .await;
        }
        let workspace = workspace.ok_or_else(|| invalid("--workspace is required"))?;
        if command == "pipeline" {
            return pipeline(workspace).await;
        }
        if command == "serve" {
            return serve(workspace, addr.unwrap_or_else(|| "127.0.0.1:4330".into())).await;
        }
        if command == "live" {
            return live(
                workspace,
                endpoint.ok_or_else(|| invalid("live requires --endpoint"))?,
                model.ok_or_else(|| invalid("live requires --model"))?,
            )
            .await;
        }
        journey(command, workspace, stage).await
    }.await;
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
