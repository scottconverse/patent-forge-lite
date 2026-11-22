use crate::*;
use futures_util::FutureExt;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    runtime::Handle,
    sync::{Mutex, mpsc, watch},
};

#[derive(Clone)]
struct Job {
    project_id: ProjectId,
    attempt_id: AttemptId,
    cancellation: CancellationToken,
    done: watch::Receiver<bool>,
}

pub struct WorkflowService {
    store: Arc<dyn Store>,
    provider: Arc<dyn Provider>,
    runtime: Handle,
    /// Optional prior-art source. Absent means no research is performed at all;
    /// it is only consulted for runs that explicitly authorized research.
    research: Option<Arc<dyn ResearchSource>>,
    commands: Mutex<()>,
    jobs: StdMutex<HashMap<RunId, Job>>,
    stopping: AtomicBool,
    started: AtomicBool,
}

impl WorkflowService {
    /// The application must retain the Tokio runtime until shutdown completes.
    /// This constructor has no recovery, network, filesystem or provider effects.
    pub fn new(store: Arc<dyn Store>, provider: Arc<dyn Provider>, runtime: Handle) -> Arc<Self> {
        Self::new_with_research(store, provider, runtime, None)
    }

    /// Adds a prior-art source without changing anything else about a run.
    pub fn new_with_research(
        store: Arc<dyn Store>,
        provider: Arc<dyn Provider>,
        runtime: Handle,
        research: Option<Arc<dyn ResearchSource>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            store,
            provider,
            runtime,
            research,
            commands: Mutex::new(()),
            jobs: StdMutex::new(HashMap::new()),
            stopping: AtomicBool::new(false),
            started: AtomicBool::new(false),
        })
    }

    async fn db<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn Store) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let store = self.store.clone();
        self.runtime
            .spawn_blocking(move || f(store.as_ref()))
            .await
            .map_err(|_| Error::Storage("storage worker stopped unexpectedly".into()))?
    }

    pub fn provider_snapshot(&self) -> ProviderSnapshot {
        self.provider.snapshot()
    }

    pub async fn create_project(&self, intake: Intake) -> Result<Project> {
        let now = now_ms();
        let project = Project {
            id: ProjectId::new(),
            intake,
            created_at_ms: now,
            updated_at_ms: now,
        };
        self.db(move |s| s.create_project(&project)).await
    }
    pub async fn projects(&self) -> Result<Vec<Project>> {
        self.db(|s| s.list_projects()).await
    }
    pub async fn project(&self, id: ProjectId) -> Result<Project> {
        self.db(move |s| s.project(&id)).await
    }
    pub async fn save_intake(&self, id: ProjectId, intake: Intake) -> Result<Project> {
        self.db(move |s| s.save_intake(&id, &intake, now_ms()))
            .await
    }
    /// Saves an edited document as a new revision. Generation runs stay immutable:
    /// a revision records the run it was edited from and never mutates it.
    pub async fn save_revision(
        &self,
        project_id: ProjectId,
        kind: DocumentKind,
        base_run_id: Option<RunId>,
        text: String,
        note: String,
    ) -> Result<DocumentRevision> {
        if let Some(run_id) = &base_run_id {
            let run = self.run(run_id.clone()).await?;
            if run.project_id != project_id || run.input.kind != kind.workflow() {
                return Err(Error::Invalid(
                    "a revision must reference a run of the same project and document".into(),
                ));
            }
        }
        let revision = DocumentRevision {
            id: RevisionId::new(),
            project_id,
            kind,
            version: 0,
            base_run_id,
            text,
            note,
            created_at_ms: now_ms(),
        };
        self.db(move |store| store.save_revision(&revision)).await
    }

    pub async fn revisions(
        &self,
        project_id: ProjectId,
        kind: DocumentKind,
    ) -> Result<Vec<DocumentRevision>> {
        let store = self.store.clone();
        let runtime = self.runtime.clone();
        runtime
            .spawn_blocking(move || store.revisions(&project_id, kind))
            .await
            .map_err(|e| Error::Storage(e.to_string()))?
    }

    pub async fn revision(&self, id: RevisionId) -> Result<DocumentRevision> {
        let store = self.store.clone();
        let runtime = self.runtime.clone();
        runtime
            .spawn_blocking(move || store.revision(&id))
            .await
            .map_err(|e| Error::Storage(e.to_string()))?
    }

    pub async fn settings(&self) -> Result<Settings> {
        self.db(|s| s.settings()).await
    }
    pub async fn save_settings(&self, settings: Settings) -> Result<()> {
        self.db(move |s| s.save_settings(&settings)).await
    }
    pub async fn run(&self, id: RunId) -> Result<RunSnapshot> {
        self.db(move |s| s.run(&id)).await
    }
    pub async fn history(&self, id: ProjectId) -> Result<Vec<RunSnapshot>> {
        self.db(move |s| s.history(&id)).await
    }
    pub async fn report(&self, id: RunId) -> Result<SavedReport> {
        let run = self.run(id).await?;
        export_markdown(&run)?;
        run.report
            .ok_or_else(|| Error::NotFound("saved report".into()))
    }
    /// Durable reconnect/polling cursor. Never creates or resumes a job.
    pub async fn events(&self, cursor: Subscribe) -> Result<Vec<RunEvent>> {
        self.db(move |s| s.events_after(&cursor)).await
    }

    fn authorize(&self, input: &RunInput) -> Result<()> {
        if input.provider.kind == ProviderKind::Cloud && !input.authorization.paid_provider {
            return Err(Error::Invalid(
                "paid/cloud providers require explicit paid-provider authorization for this run"
                    .into(),
            ));
        }
        if input.provider != self.provider.snapshot() {
            return Err(Error::Conflict(
                "provider snapshot differs; restore the exact adapter/endpoint/model before resume"
                    .into(),
            ));
        }
        if input.prompts != prompts::snapshot(input.kind) {
            return Err(Error::Conflict(
                "prompt snapshot differs; restore the exact prompt version before resume".into(),
            ));
        }
        // Every run needs an explicit data declaration: either synthetic test
        // material, or an acknowledgement that this is a real invention. The
        // declaration is part of the immutable run input.
        if !input.authorization.synthetic_data && !input.authorization.real_data {
            return Err(Error::Invalid(
                "declare the run's data first: synthetic test data, or a real-invention acknowledgement"
                    .into(),
            ));
        }
        if input.authorization.synthetic_data && input.authorization.real_data {
            return Err(Error::Invalid(
                "a run holds exactly one data declaration: synthetic test data or a real invention, not both"
                    .into(),
            ));
        }
        // Real-invention data is Local-only until a Cloud consent model is designed.
        if input.authorization.real_data && input.provider.kind == ProviderKind::Cloud {
            return Err(Error::Invalid(
                "real-invention data is local-only in this build; cloud runs require synthetic test data"
                    .into(),
            ));
        }
        if input.provider.kind == ProviderKind::Replay && !input.authorization.synthetic_data {
            return Err(Error::Invalid(
                "recorded replay produces synthetic output and cannot be used for a real-invention run"
                    .into(),
            ));
        }
        if input.provider.kind == ProviderKind::Local && !input.authorization.local_inference {
            return Err(Error::Invalid(
                "local inference has not been explicitly authorized for this endpoint and model"
                    .into(),
            ));
        }
        if input.intake.title.trim().is_empty()
            || input.intake.description.split_whitespace().count() < 50
        {
            return Err(Error::Invalid(
                "invention title and description of at least 50 words required".into(),
            ));
        }
        Ok(())
    }

    pub async fn start_project(
        self: &Arc<Self>,
        project_id: ProjectId,
        idempotency_key: String,
        authorization: Authorization,
    ) -> Result<RunSnapshot> {
        self.start_workflow(
            project_id,
            WorkflowKind::Feasibility,
            idempotency_key,
            authorization,
        )
        .await
    }

    /// Starts one workflow. A dependent workflow snapshots the exact upstream
    /// text it consumes, so a later upstream edit cannot change this run.
    pub async fn start_workflow(
        self: &Arc<Self>,
        project_id: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
    ) -> Result<RunSnapshot> {
        let project = self.project(project_id.clone()).await?;
        let mut upstream = self.upstream(&project_id, kind).await?;
        // Setup profiles decide which provider families an installation may use.
        if self.settings().await?.profile == SetupProfile::Lean
            && self.provider.snapshot().kind == ProviderKind::Local
        {
            return Err(Error::Invalid(
                "Lean setup is Cloud-only: it never acquires or uses a local runtime".into(),
            ));
        }
        if let Some(research) = &self.research
            && authorization.research
            && kind == WorkflowKind::Feasibility
        {
            upstream.prior_art = research
                .prior_art(&project.intake, CancellationToken::new())
                .await?;
        }
        self.start(NewRun {
            id: RunId::new(),
            project_id,
            idempotency_key,
            input: RunInput {
                kind,
                intake: project.intake,
                provider: self.provider.snapshot(),
                prompts: prompts::snapshot(kind),
                authorization,
                upstream,
                regeneration: None,
            },
            rerun_of: None,
            created_at_ms: now_ms(),
        })
        .await
    }

    /// Resolves and snapshots the upstream text a workflow consumes. Requires a
    /// completed run of every declared prerequisite workflow.
    pub async fn upstream(&self, project: &ProjectId, kind: WorkflowKind) -> Result<Upstream> {
        let history = self.history(project.clone()).await?;
        let mut upstream = Upstream::default();
        for required in kind.requires() {
            let run = history
                .iter()
                .filter(|run| run.input.kind == *required && run.outcome == RunOutcome::Completed)
                .max_by_key(|run| run.version)
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "{} must complete a {} run before this workflow can start",
                        required.label(),
                        required.slug()
                    ))
                })?;
            for stage in kind.upstream_stages() {
                if stage.kind() != *required {
                    continue;
                }
                let text = run
                    .checkpoints
                    .iter()
                    .find(|checkpoint| checkpoint.stage == *stage)
                    .map(|checkpoint| checkpoint.text.clone())
                    .unwrap_or_default();
                upstream.stage_texts.push((*stage, text));
            }
            if *required == WorkflowKind::Claims {
                upstream.claims_text = run
                    .checkpoints
                    .iter()
                    .filter(|checkpoint| checkpoint.stage.kind() == WorkflowKind::Claims)
                    .max_by_key(|checkpoint| checkpoint.stage.number())
                    .map(|checkpoint| checkpoint.text.clone())
                    .unwrap_or_default();
            }
            if *required == WorkflowKind::Feasibility {
                upstream.specification_text = run
                    .report
                    .as_ref()
                    .map(|r| r.markdown.clone())
                    .unwrap_or_default();
            }
        }
        // Dependent workflows inherit the reference set their prerequisite recorded.
        // A fresh feasibility run must not inherit a previous feasibility's set: that
        // would attach references the operator never chose for this run.
        if kind != WorkflowKind::Feasibility {
            upstream.prior_art = self.prior_art(&history);
        }
        Ok(upstream)
    }

    /// Live research is not implemented in this product yet, so a project run
    /// records no prior-art sources of its own. Callers can still supply explicit
    /// references when starting a dependent workflow.
    /// The reference set a dependent workflow inherits: whatever the most recent
    /// completed feasibility run recorded. Stored in the run input, so it can never
    /// drift or be re-queried on resume.
    fn prior_art(&self, history: &[RunSnapshot]) -> Vec<PriorArtItem> {
        history
            .iter()
            .filter(|run| {
                run.input.kind == WorkflowKind::Feasibility && run.outcome == RunOutcome::Completed
            })
            .max_by_key(|run| run.version)
            .map(|run| run.input.upstream.prior_art.clone())
            .unwrap_or_default()
    }
    pub async fn start_workflow_with_prior_art(
        self: &Arc<Self>,
        project_id: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
        prior_art: Vec<PriorArtItem>,
    ) -> Result<RunSnapshot> {
        let project = self.project(project_id.clone()).await?;
        let mut upstream = self.upstream(&project_id, kind).await?;
        if self.settings().await?.profile == SetupProfile::Lean
            && self.provider.snapshot().kind == ProviderKind::Local
        {
            return Err(Error::Invalid(
                "Lean setup is Cloud-only: it never acquires or uses a local runtime".into(),
            ));
        }
        upstream.prior_art = prior_art;
        self.start(NewRun {
            id: RunId::new(),
            project_id,
            idempotency_key,
            input: RunInput {
                kind,
                intake: project.intake,
                provider: self.provider.snapshot(),
                prompts: prompts::snapshot(kind),
                authorization,
                upstream,
                regeneration: None,
            },
            rerun_of: None,
            created_at_ms: now_ms(),
        })
        .await
    }

    pub async fn execute(self: &Arc<Self>, command: Command) -> Result<RunSnapshot> {
        match command {
            Command::Start(request) => self.start(request).await,
            Command::Resume(id) => self.resume(id).await,
            Command::Rerun { source, request } => self.rerun(source, request).await,
            Command::Cancel(id) => self.cancel(id).await,
        }
    }

    pub async fn start(self: &Arc<Self>, request: NewRun) -> Result<RunSnapshot> {
        let _commands = self.commands.lock().await;
        self.start_locked(request).await
    }

    async fn start_locked(self: &Arc<Self>, request: NewRun) -> Result<RunSnapshot> {
        if self.stopping.load(Ordering::Acquire) {
            return Err(Error::Conflict("application is shutting down".into()));
        }
        self.authorize(&request.input)?;
        if request.idempotency_key.trim().is_empty() {
            return Err(Error::Invalid("idempotency key is required".into()));
        }
        let history = self.history(request.project_id.clone()).await?;
        if history
            .iter()
            .any(|r| r.idempotency_key == request.idempotency_key)
        {
            // Store verifies identical immutable payload, not just key equality.
            return self.db(move |s| s.create_run(&request)).await;
        }
        if self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|j| j.project_id == request.project_id)
        {
            return Err(Error::Conflict(
                "project already has an owned active attempt; await cancellation first".into(),
            ));
        }
        if history
            .iter()
            .any(|r| matches!(r.outcome, RunOutcome::Running | RunOutcome::Cancelling))
        {
            return Err(Error::Conflict("project has an interrupted or externally owned active attempt; startup recovery is required".into()));
        }
        let run = self.db(move |s| s.create_run(&request)).await?;
        self.launch(run).await
    }

    /// Validates a section selection against a completed run and returns the plan a
    /// regeneration would execute. Nothing runs here: the plan only names stages the
    /// base actually produced, in the base's own order.
    pub async fn plan_regeneration(&self, source: RunId, stages: Vec<Stage>) -> Result<Vec<Stage>> {
        if stages.is_empty() {
            return Err(Error::Invalid(
                "a regeneration must name at least one stage".into(),
            ));
        }
        let available = self.regenerable_stages(source).await?;
        for stage in &stages {
            if !available.contains(stage) {
                return Err(Error::Invalid(format!(
                    "stage {stage:?} is not part of the base run"
                )));
            }
        }
        // Deduplicate and keep the base run's order, so a plan is deterministic.
        let mut plan: Vec<Stage> = Vec::new();
        for stage in available {
            if stages.contains(&stage) && !plan.contains(&stage) {
                plan.push(stage);
            }
        }
        Ok(plan)
    }

    /// The stages of a completed run that a section-scoped regeneration may re-run.
    /// Only committed checkpoints are offered, so a selection can never name a stage
    /// the base run never produced.
    pub async fn regenerable_stages(&self, id: RunId) -> Result<Vec<Stage>> {
        let run = self.run(id).await?;
        if !run.outcome.is_terminal() {
            return Err(Error::Conflict(
                "the source run must be terminal before it can be regenerated".into(),
            ));
        }
        Ok(run
            .checkpoints
            .iter()
            .map(|checkpoint| checkpoint.stage)
            .collect())
    }

    /// Starts a dependent workflow against *selected document revisions* rather than
    /// whatever the project happens to have produced last. The selection is recorded
    /// in the immutable upstream snapshot so a review can always name what it read.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_workflow_with_selection(
        self: &Arc<Self>,
        project_id: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
        prior_art: Vec<PriorArtItem>,
        claims_revision: Option<RevisionId>,
        specification_revision: Option<RevisionId>,
    ) -> Result<RunSnapshot> {
        let project = self.project(project_id.clone()).await?;
        let mut upstream = self.upstream(&project_id, kind).await?;
        if self.settings().await?.profile == SetupProfile::Lean
            && self.provider.snapshot().kind == ProviderKind::Local
        {
            return Err(Error::Invalid(
                "Lean setup is Cloud-only: it never acquires or uses a local runtime".into(),
            ));
        }
        upstream.prior_art = prior_art;
        if let Some(revision_id) = claims_revision {
            let revision = self.revision(revision_id.clone()).await?;
            if revision.project_id != project_id || revision.kind != DocumentKind::Claims {
                return Err(Error::Invalid(
                    "the selected claims revision belongs to another project or document".into(),
                ));
            }
            upstream.claims_text = revision.text;
            upstream.claims_revision_id = Some(revision_id);
        }
        if let Some(revision_id) = specification_revision {
            let revision = self.revision(revision_id.clone()).await?;
            if revision.project_id != project_id || revision.kind != DocumentKind::Application {
                return Err(Error::Invalid(
                    "the selected specification revision belongs to another project or document"
                        .into(),
                ));
            }
            upstream.specification_text = revision.text;
            upstream.specification_revision_id = Some(revision_id);
        }
        self.start(NewRun {
            id: RunId::new(),
            project_id,
            idempotency_key,
            input: RunInput {
                kind,
                intake: project.intake,
                provider: self.provider.snapshot(),
                prompts: prompts::snapshot(kind),
                authorization,
                upstream,
                regeneration: None,
            },
            rerun_of: None,
            created_at_ms: now_ms(),
        })
        .await
    }

    pub async fn resume(self: &Arc<Self>, id: RunId) -> Result<RunSnapshot> {
        let _commands = self.commands.lock().await;
        if self.stopping.load(Ordering::Acquire) {
            return Err(Error::Conflict("application is shutting down".into()));
        }
        let run = self.run(id.clone()).await?;
        if self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&id)
        {
            return Err(Error::Conflict(
                "attempt is still active; cancellation must finish before replacement".into(),
            ));
        }
        if run.outcome == RunOutcome::Completed {
            return Ok(run);
        }
        self.authorize(&run.input)?;
        if self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|j| j.project_id == run.project_id)
        {
            return Err(Error::Conflict(
                "project already has another active attempt".into(),
            ));
        }
        self.launch(run).await
    }

    /// A full six-stage rerun creates a new immutable version. Stage-selective
    /// rerun is later parity; never mutate the original run or its saved report.
    pub async fn rerun(
        self: &Arc<Self>,
        source: RunId,
        mut request: NewRun,
    ) -> Result<RunSnapshot> {
        let _commands = self.commands.lock().await;
        let original = self.run(source.clone()).await?;
        if original.project_id != request.project_id || original.id == request.id {
            return Err(Error::Invalid(
                "rerun must use same project and a new run identity".into(),
            ));
        }
        if !original.outcome.is_terminal() {
            return Err(Error::Conflict("source run is not terminal".into()));
        }
        request.rerun_of = Some(source);
        self.start_locked(request).await
    }

    async fn launch(self: &Arc<Self>, run: RunSnapshot) -> Result<RunSnapshot> {
        // Every caller holds `commands` until registration is complete. Shutdown
        // takes the same gate before closing admission and collecting jobs.
        if self.stopping.load(Ordering::Acquire) {
            return Err(Error::Conflict("application is shutting down".into()));
        }
        let id = run.id.clone();
        let attempt = self.db(move |s| s.begin_attempt(&id, now_ms())).await?;
        self.started.store(true, Ordering::Release);
        let cancellation = CancellationToken::new();
        let (finished, done) = watch::channel(false);
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).insert(
            run.id.clone(),
            Job {
                project_id: run.project_id.clone(),
                attempt_id: attempt.id.clone(),
                cancellation: cancellation.clone(),
                done,
            },
        );
        let service = self.clone();
        let run_id = run.id.clone();
        // Returning/dropping a view cannot drop this application-owned task.
        self.runtime.spawn(async move {
            let result = std::panic::AssertUnwindSafe(service.drive(&run, &attempt, &cancellation))
                .catch_unwind()
                .await
                .unwrap_or_else(|_| {
                    Err(Error::Provider(
                        "provider execution stopped unexpectedly".into(),
                    ))
                });
            let (outcome, report) = match result {
                Ok(report) if !cancellation.is_cancelled() => (RunOutcome::Completed, Some(report)),
                Ok(_) | Err(Error::Cancelled) => (RunOutcome::Cancelled, None),
                Err(_) if cancellation.is_cancelled() => (RunOutcome::Cancelled, None),
                // A stream that dies before its terminal frame is not a failure of
                // the analysis; it is an incomplete answer and is recorded as such.
                Err(Error::Incomplete(_)) => (RunOutcome::Truncated, None),
                Err(_) => (RunOutcome::Failed, None),
            };
            // The Store's compound terminal write owns the sanitized durable
            // failure notice; append_event may not forge outcome transitions.
            let id = run.id.clone();
            let aid = attempt.id.clone();
            let terminal_token = cancellation.clone();
            let finalization = service
                .db(move |s| {
                    let terminal = if terminal_token.is_cancelled() {
                        RunOutcome::Cancelled
                    } else {
                        outcome
                    };
                    let saved = if terminal == RunOutcome::Completed {
                        report.as_ref()
                    } else {
                        None
                    };
                    let result = s.finish_attempt(&id, &aid, terminal, saved, now_ms());
                    if result.is_err() && terminal_token.is_cancelled() {
                        s.finish_attempt(&id, &aid, RunOutcome::Cancelled, None, now_ms())
                    } else {
                        result
                    }
                })
                .await;
            // A failed terminal write must remain recoverable, never reported
            // completed solely from transient provider output.
            if finalization.is_err() {
                cancellation.cancel();
            }
            service
                .jobs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&run.id);
            let _ = finished.send(true);
        });
        self.run(run_id).await
    }

    async fn event(&self, run: &RunId, attempt: &AttemptId, event: EventKind) -> Result<()> {
        let (run, attempt) = (run.clone(), attempt.clone());
        self.db(move |s| s.append_event(&run, &attempt, event, now_ms()))
            .await?;
        Ok(())
    }

    async fn drive(
        &self,
        run: &RunSnapshot,
        attempt: &Attempt,
        cancellation: &CancellationToken,
    ) -> Result<SavedReport> {
        let mut checkpoints = run.checkpoints.clone();
        checkpoints.sort_by_key(|cp| cp.stage);
        let stages = run.input.kind.stages();
        for (index, cp) in checkpoints.iter().enumerate() {
            if cp.stage != stages[index.min(stages.len() - 1)]
                || index >= stages.len()
                || cp.stage.kind() != run.input.kind
            {
                return Err(Error::Conflict(
                    "checkpoints must form a contiguous completed prefix".into(),
                ));
            }
        }
        // "Regenerate from here onward": stages before the restart point are carried
        // from the base run verbatim, so they stay byte-identical and are never
        // re-generated. The prefix stays contiguous, which the resume guarantee needs.
        if let Some(plan) = run.input.regeneration.clone() {
            let Some(restart) = stages.iter().position(|stage| *stage == plan.from_stage) else {
                return Err(Error::Invalid(
                    "the restart stage is not part of this workflow".into(),
                ));
            };
            let base = self.run(plan.base_run.clone()).await?;
            let mut carried: Vec<Checkpoint> = base
                .checkpoints
                .iter()
                .filter(|checkpoint| {
                    stages
                        .iter()
                        .position(|stage| *stage == checkpoint.stage)
                        .map(|index| index < restart)
                        .unwrap_or(false)
                })
                .cloned()
                .collect();
            carried.sort_by_key(|checkpoint| checkpoint.stage);
            if carried.len() != restart {
                return Err(Error::Conflict(
                    "the base run did not complete every stage before the restart point".into(),
                ));
            }
            for original in carried {
                let checkpoint = Checkpoint {
                    stage: original.stage,
                    attempt_id: attempt.id.clone(),
                    text: original.text,
                    usage: original.usage,
                    completed_at_ms: original.completed_at_ms,
                };
                let id = run.id.clone();
                let aid = attempt.id.clone();
                let cp = checkpoint.clone();
                self.db(move |s| s.commit_checkpoint(&id, &aid, &cp))
                    .await?;
                checkpoints.push(checkpoint);
            }
        }
        for stage in stages.iter().copied().skip(checkpoints.len()) {
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            self.event(&run.id, &attempt.id, EventKind::StageStarted { stage })
                .await?;
            let context_checkpoints = checkpoints.clone();
            let context = self
                .runtime
                .spawn_blocking(move || crate::context::stage_context(stage, &context_checkpoints))
                .await
                .map_err(|_| Error::Storage("context worker stopped unexpectedly".into()))??;
            let request = ProviderRequest {
                run_id: run.id.clone(),
                attempt_id: attempt.id.clone(),
                stage,
                input: run.input.clone(),
                system_prompt: prompts::system(stage),
                user_prompt: prompts::user(stage, &run.input, &context),
                prior_outputs: checkpoints.clone(),
            };
            let (tx, mut rx) = mpsc::unbounded_channel();
            let provider = self.provider.clone();
            let generation = provider.generate(request, cancellation.clone(), tx);
            tokio::pin!(generation);
            let mut channel_open = true;
            let response = loop {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => return Err(Error::Cancelled),
                    next = rx.recv(), if channel_open => {
                        if let Some(text) = next {
                            self.event(&run.id, &attempt.id, EventKind::TextDelta { stage, text }).await?;
                        } else { channel_open = false; }
                    },
                    result = &mut generation => break result?
                }
            };
            while let Ok(text) = rx.try_recv() {
                if cancellation.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                self.event(&run.id, &attempt.id, EventKind::TextDelta { stage, text })
                    .await?;
            }
            if cancellation.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if response.text.trim().is_empty() {
                return Err(Error::Provider("empty stage response".into()));
            }
            let checkpoint = Checkpoint {
                stage,
                attempt_id: attempt.id.clone(),
                text: response.text,
                usage: response.usage,
                completed_at_ms: now_ms(),
            };
            let id = run.id.clone();
            let aid = attempt.id.clone();
            let cp = checkpoint.clone();
            self.db(move |s| s.commit_checkpoint(&id, &aid, &cp))
                .await?;
            checkpoints.push(checkpoint);
        }
        let markdown = crate::report::compose(
            run.input.kind,
            &run.input.intake.title,
            &checkpoints,
            &run.input.upstream,
        )?;
        Ok(SavedReport {
            run_id: run.id.clone(),
            version: run.version,
            markdown,
        })
    }

    /// "Regenerate from here onward": a new run that re-runs `from_stage` and every
    /// later stage of the workflow, carrying the earlier stages from the base run.
    /// Suffix only -- there is no single-stage regeneration, by decision.
    pub async fn regenerate_from(
        self: &Arc<Self>,
        source: RunId,
        from_stage: Stage,
    ) -> Result<RunSnapshot> {
        let _commands = self.commands.lock().await;
        let base = self.run(source.clone()).await?;
        if !base.outcome.is_terminal() {
            return Err(Error::Conflict(
                "the base run must be terminal before it can be regenerated".into(),
            ));
        }
        if !base.input.kind.stages().contains(&from_stage) {
            return Err(Error::Invalid(
                "the restart stage is not part of this workflow".into(),
            ));
        }
        if !base
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.stage == from_stage)
        {
            return Err(Error::Invalid(
                "the base run did not complete the restart stage".into(),
            ));
        }
        let mut input = base.input.clone();
        input.regeneration = Some(RegenerationPlan {
            base_run: source.clone(),
            from_stage,
        });
        self.start_locked(NewRun {
            id: RunId::new(),
            project_id: base.project_id.clone(),
            idempotency_key: format!("regenerate:{}", RunId::new().0),
            input,
            rerun_of: Some(source),
            created_at_ms: now_ms(),
        })
        .await
    }

    /// Returns only after the owned provider future is stopped and a durable
    /// terminal outcome is visible. No replacement attempt can overlap it.
    pub async fn cancel(self: &Arc<Self>, id: RunId) -> Result<RunSnapshot> {
        let _commands = self.commands.lock().await;
        let job = self
            .jobs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned();
        let Some(mut job) = job else {
            let run = self.run(id).await?;
            if run.outcome.is_terminal() {
                return Ok(run);
            }
            return Err(Error::Conflict(
                "run has no locally owned attempt; recover startup interruption first".into(),
            ));
        };
        let rid = id.clone();
        let aid = job.attempt_id.clone();
        job.cancellation.cancel();
        let cancelling = self
            .db(move |s| s.set_cancelling(&rid, &aid, now_ms()))
            .await;
        // Even a failed cancelling write must stop the owned request.
        while !*job.done.borrow() {
            if job.done.changed().await.is_err() {
                break;
            }
        }
        let run = self.run(id).await?;
        if run.outcome.is_terminal() {
            Ok(run)
        } else {
            cancelling?;
            Err(Error::Storage("cancellation stopped execution but terminal persistence failed; startup recovery required".into()))
        }
    }

    /// Startup-only, before accepting commands. Reads never invoke recovery.
    pub async fn recover_interrupted(&self) -> Result<Vec<RunId>> {
        let _commands = self.commands.lock().await;
        if self.started.load(Ordering::Acquire)
            || !self
                .jobs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        {
            return Err(Error::Conflict(
                "startup recovery cannot run after this service starts jobs".into(),
            ));
        }
        self.db(|s| s.recover_interrupted(now_ms())).await
    }

    pub async fn shutdown(self: &Arc<Self>) -> Result<()> {
        let ids: Vec<_> = {
            let _commands = self.commands.lock().await;
            self.stopping.store(true, Ordering::Release);
            self.jobs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .keys()
                .cloned()
                .collect()
        };
        // cancel takes `commands`; never retain the admission gate while
        // awaiting job completion, and never let completion acquire that gate.
        let mut first_error = None;
        for id in ids {
            if let Err(e) = self.cancel(id).await
                && first_error.is_none()
            {
                first_error = Some(e);
            }
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }
}
