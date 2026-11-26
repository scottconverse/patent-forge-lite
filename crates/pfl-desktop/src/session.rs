//! App-lifetime async bridge. Only explicit requests can create or resume jobs.
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::{
    CloudProvider, OllamaProvider, PatentsViewSource, ReplayProvider, ResearchSource,
    WorkflowService, export_html, export_markdown, providers::OpenAiCompatibleProvider,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
};

pub const APPLICATION_ID: &str = "com.scottconverse.patentforgelite";
pub const APPLICATION_NAME: &str = "Patent Forge Lite";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Explicit fixture execution, never a default live-provider substitute.
    Replay,
    Local {
        endpoint: String,
        model: String,
    },
    /// Anthropic Messages. Selecting this sends nothing; it only records which
    /// provider family a later run may use. The key lives in memory for the
    /// session and is never written to the profile, a snapshot, an event or an
    /// export.
    Cloud {
        model: String,
    },
}

#[derive(Clone, Copy, Debug)]
pub enum ExportFormat {
    Markdown,
    Html,
    Word,
}

/// A session-only secret. Its Debug prints a marker, so a future {:?} on a request cannot put
/// a key in a log.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        // A key pasted from a password manager often carries a newline or a trailing space,
        // which produces an authentication failure with no visible cause.
        Self(value.into().trim().to_string())
    }
    /// The only way to read the value. Deliberately not Deref, so a key does not silently
    /// behave like a string in formatting code.
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_blank(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\"[redacted]\"")
    }
}

#[derive(Debug)]
pub enum Request {
    Refresh,
    Create(Intake),
    Open(ProjectId),
    Save(ProjectId, Intake),
    SelectRun(RunId),
    Configure(Mode),
    /// Configures USPTO PatentsView research with a session-only key.
    ConfigureResearch {
        api_key: Secret,
    },
    /// Holds the Cloud key in memory for this session only. A `None` clears the stored key,
    /// but a provider already built keeps the key it was built with until the next
    /// `ConfigureCloud`; clearing does not revoke a live provider. The value is never stored,
    /// logged, exported or echoed back in a reply.
    SetCloudKey(Option<Secret>),
    /// Configures Cloud in one step: the key is installed and the mode is built
    /// together, so no ordering race can leave the mode unset or the key missing.
    ConfigureCloud {
        model: String,
        key: Secret,
    },
    SetProfile(SetupProfile),
    Start {
        project: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
    },
    /// Resuming spends again, so it needs the authorization for the retry.
    Resume {
        run: RunId,
        authorization: Authorization,
    },
    Cancel(RunId),
    Export {
        run: RunId,
        path: PathBuf,
        format: ExportFormat,
    },
    /// Lists the models a loopback runtime reports. An unreachable runtime is an error,
    /// never an empty list.
    /// Ask for the whole catalogue: every provider, what it has, and whether it is usable.
    DiscoverProviders,
    /// Which provider runs each surface for this project.
    ListAssignments {
        project: ProjectId,
    },
    /// Rank 0 is the choice, 1 and 2 are the fallbacks. An empty provider clears the rank.
    SetAssignment {
        project: ProjectId,
        surface: String,
        rank: u8,
        provider: String,
        model: Option<String>,
    },
    ClearAssignments {
        project: ProjectId,
        surface: String,
    },
    ListLocalModels {
        endpoint: String,
    },
    /// Re-runs the given stage and every later stage of that run, carrying the earlier
    /// stages from the base run. Suffix only.
    RegenerateFrom {
        run: RunId,
        stage: Stage,
        authorization: Authorization,
    },
    /// Writes the package for the chosen revisions (md, html, docx, csv, manifest) into
    /// a directory and reports it.
    ExportPackage {
        project: ProjectId,
        claims: Option<RevisionId>,
        application: Option<RevisionId>,
        dir: PathBuf,
    },
    /// Starts a dependent workflow against explicitly selected document revisions.
    StartSelected {
        project: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
        claims_revision: Option<RevisionId>,
        specification_revision: Option<RevisionId>,
    },
    /// Saves edited document text as the next revision, anchored to the latest
    /// completed run of that document's kind.
    SaveRevision {
        project: ProjectId,
        kind: DocumentKind,
        text: String,
        note: String,
    },
    /// Lists the stored revisions of one document kind for a project.
    ListRevisions {
        project: ProjectId,
        kind: DocumentKind,
    },
    Shutdown,
}

#[derive(Debug)]
pub enum Reply {
    Projects(Vec<Project>),
    Opened(Project, Vec<RunSnapshot>),
    Saved(Project),
    Run(RunSnapshot),
    Poll {
        events: Vec<RunEvent>,
        snapshot: RunSnapshot,
    },
    Configured(Mode),
    Settings(Settings),
    Exported(PathBuf),
    Revision(DocumentRevision),
    PackageExported(PathBuf),
    ResearchConfigured,
    LocalModels(Vec<String>),
    /// The project's model assignments, ordered by surface and rank.
    Assignments(Vec<ModelAssignment>),
    /// The joined catalogue: registry, discovery and sign-in state.
    Providers(Vec<pfl_workflow::catalog::ProviderView>),
    Revisions(Vec<DocumentRevision>),
    Recovered(Vec<RunId>),
    Error(String),
    Stopped,
}

/// The runtime and runner outlive individual GPUI windows and navigation.
/// True only for a loopback HTTP endpoint. Used to keep the verification hook from
/// redirecting a real Cloud key anywhere but this machine.
fn is_loopback_endpoint(endpoint: &str) -> bool {
    let Some(rest) = endpoint.strip_prefix("http://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let host = if let Some(inner) = authority.strip_prefix('[') {
        inner.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

pub struct ApplicationSession {
    runtime: Arc<tokio::runtime::Runtime>,
    sender: tokio::sync::mpsc::UnboundedSender<Request>,
    replies: mpsc::Receiver<Reply>,
    stopped: Arc<AtomicBool>,
    active_jobs: Arc<AtomicUsize>,
    instance_lock: Option<std::fs::File>,
}

impl ApplicationSession {
    pub fn open(database: impl AsRef<Path>) -> Result<Self> {
        let database = database.as_ref();
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(database.with_extension("lock"))
            .map_err(|e| Error::Io(e.to_string()))?;
        lock.try_lock().map_err(|_| {
            Error::Conflict(
                "another Patent Forge Lite window is already using this data location; \
                 use that window, because only one instance may write to the workspace"
                    .into(),
            )
        })?;
        let store: Arc<dyn Store> = Arc::new(SqliteStore::open(database)?);
        let mut session = Self::with_store(store)?;
        session.instance_lock = Some(lock);
        Ok(session)
    }

    pub fn with_store(store: Arc<dyn Store>) -> Result<Self> {
        // Recorded replay must cover every workflow's stages, not only
        // feasibility, or starting a claims/compliance/application run in replay
        // mode would fail with "recorded stage missing".
        Self::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
    }

    /// Dependency injection for deterministic replay acceptance. Live providers
    /// cannot enter through this constructor and configuration still grants no run.
    pub fn with_replay_provider(store: Arc<dyn Store>, replay: Arc<dyn Provider>) -> Result<Self> {
        if replay.snapshot().kind != ProviderKind::Replay {
            return Err(Error::Invalid(
                "The replay adapter must identify itself as recorded replay".into(),
            ));
        }
        let runtime =
            Arc::new(tokio::runtime::Runtime::new().map_err(|e| Error::Io(e.to_string()))?);
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let (replies, inbox) = mpsc::channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let active_jobs = Arc::new(AtomicUsize::new(0));
        let service = WorkflowService::new(store.clone(), replay.clone(), runtime.handle().clone());
        runtime.spawn(
            Actor {
                store,
                service,
                receiver,
                replies,
                mode: None,
                cursor: None,
                started_runs: HashSet::new(),
                stopped: stopped.clone(),
                active_jobs: active_jobs.clone(),
                replay,
                research: None,
                cloud_key: None,
                research_configured: false,
            }
            .run(),
        );
        Ok(Self {
            runtime,
            sender,
            replies: inbox,
            stopped,
            active_jobs,
            instance_lock: None,
        })
    }

    pub fn request(&self, request: Request) -> Result<()> {
        self.sender
            .send(request)
            .map_err(|_| Error::Conflict("application session has stopped".into()))
    }

    pub fn drain(&self) -> Vec<Reply> {
        self.replies.try_iter().collect()
    }

    pub fn runtime(&self) -> &Arc<tokio::runtime::Runtime> {
        &self.runtime
    }
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    /// Every job this application still owns, across every project and every
    /// historical run, not only the run currently selected in the window.
    pub fn active_jobs(&self) -> usize {
        self.active_jobs.load(Ordering::Acquire)
    }
}

impl Drop for ApplicationSession {
    fn drop(&mut self) {
        let _ = self.sender.send(Request::Shutdown);
    }
}

struct Actor {
    store: Arc<dyn Store>,
    service: Arc<WorkflowService>,
    receiver: tokio::sync::mpsc::UnboundedReceiver<Request>,
    replies: mpsc::Sender<Reply>,
    mode: Option<Mode>,
    cursor: Option<Subscribe>,
    started_runs: HashSet<RunId>,
    stopped: Arc<AtomicBool>,
    active_jobs: Arc<AtomicUsize>,
    replay: Arc<dyn Provider>,
    /// Configured prior-art source. It is kept here rather than only inside the service,
    /// because changing the execution mode replaces the service and used to drop research
    /// while the window still believed it was on.
    research: Option<Arc<dyn ResearchSource>>,
    /// In-memory only, for this session. Never persisted, never logged.
    cloud_key: Option<Secret>,
    /// True once a research source has been configured for this session.
    research_configured: bool,
}

impl Actor {
    fn reply(&self, reply: Reply) {
        let _ = self.replies.send(reply);
    }

    async fn run(mut self) {
        // Exactly once, before this instance accepts commands. Opening a project
        // and subscribing must never recover or interrupt jobs.
        match self.service.recover_interrupted().await {
            Ok(ids) => self.reply(Reply::Recovered(ids)),
            Err(error) => {
                self.reply(Reply::Error(error.to_string()));
                self.stopped.store(true, Ordering::Release);
                return;
            }
        }
        if let Ok(projects) = self.service.projects().await {
            self.reply(Reply::Projects(projects));
        }
        if let Ok(settings) = self.service.settings().await {
            self.reply(Reply::Settings(settings));
        }
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(120));
        loop {
            tokio::select! {
                request = self.receiver.recv() => {
                    let Some(request) = request else { break; };
                    if matches!(request, Request::Shutdown) { break; }
                    if let Err(error) = self.handle(request).await {
                        self.reply(Reply::Error(error.to_string()));
                    }
                    self.refresh_active().await;
                }
                _ = interval.tick() => {
                    if let Err(error) = self.poll().await {
                        self.reply(Reply::Error(error.to_string()));
                        self.cursor = None;
                    }
                    self.refresh_active().await;
                }
            }
        }
        if let Err(error) = self.service.shutdown().await {
            self.reply(Reply::Error(error.to_string()));
        }
        self.stopped.store(true, Ordering::Release);
        self.reply(Reply::Stopped);
    }

    async fn selected(&mut self, snapshot: RunSnapshot) {
        self.cursor = Some(Subscribe {
            run_id: snapshot.id.clone(),
            after_sequence: 0,
        });
        self.reply(Reply::Run(snapshot));
    }

    /// Builds the provider for a mode. Cloud accepts only a loopback verification
    /// endpoint from the environment; anything else is refused, so a real key can
    /// never be redirected off this machine.
    /// Refuses when anything this session started is still running.
    ///
    /// Every path that starts work, or that replaces the service those jobs live in, must
    /// pass through this one check: a second paid run must not start beside the first, and a
    /// job must not be orphaned by a service swap that cannot cancel it.
    async fn ensure_idle(&self, action: &str) -> Result<()> {
        for id in &self.started_runs {
            if !self.service.run(id.clone()).await?.outcome.is_terminal() {
                return Err(Error::Conflict(format!(
                    "finish or cancel active work before {action}"
                )));
            }
        }
        Ok(())
    }

    /// Rebinds the service to a provider while keeping the configured research source.
    fn rebuild_service(&mut self, provider: Arc<dyn Provider>) {
        self.service = WorkflowService::new_with_research(
            self.store.clone(),
            provider,
            tokio::runtime::Handle::current(),
            self.research.clone(),
        );
    }

    /// The provider an assignment names, when it names one this build can use safely.
    ///
    /// Only local servers for now: they are loopback, so a run's text does not leave the machine.
    /// A subscription or hosted assignment needs the consent matrix finished first, and building
    /// that provider before the matrix exists would let a real invention reach a third party.
    async fn assigned_provider(
        &self,
        project: &ProjectId,
        kind: WorkflowKind,
    ) -> Result<Option<Arc<dyn Provider>>> {
        let surface = format!("workflow-{}", kind.slug());
        let rows = self.store.assignments(project)?;
        let Some(row) = rows
            .iter()
            .find(|row| row.surface == surface && row.rank == 0)
        else {
            return Ok(None);
        };
        self.provider_for_assignment(row).await
    }

    /// The provider one assignment names, when this build can honour it.
    async fn provider_for_assignment(
        &self,
        row: &pfl_domain::ModelAssignment,
    ) -> Result<Option<Arc<dyn Provider>>> {
        if let Some(program) = assigned_cli_kind(&row.provider) {
            let model = row
                .model
                .clone()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| default_model(&row.provider));
            // These kinds leave the machine; the workflow's consent rule refuses real-invention
            // data before the program is ever started, so building it here is safe.
            return Ok(Some(match program {
                AssignedCli::Claude => Arc::new(pfl_workflow::cli::ClaudeCliProvider::new(model)?),
                AssignedCli::Codex => Arc::new(pfl_workflow::cli::CodexCliProvider::new(model)?),
                AssignedCli::Gemini => Arc::new(pfl_workflow::cli::GeminiCliProvider::new(model)?),
            }));
        }
        // A saved connection is addressed by its id: an assignment naming `conn-<id>` uses exactly
        // that endpoint. The key reference is deliberately not resolved here — a connection whose
        // key lives in the OS store is wired when the connection UI lands, and until then it can
        // only serve a loopback endpoint, where no key is needed.
        if row.provider.starts_with("conn-") {
            let wanted = row.provider.clone();
            let Some(connection) = self
                .store
                .connections()?
                .into_iter()
                .find(|connection| connection.id == wanted)
            else {
                return Ok(None);
            };
            let model = row
                .model
                .clone()
                .filter(|model| !model.trim().is_empty())
                .or_else(|| connection.model.clone())
                .filter(|model| !model.trim().is_empty());
            let Some(model) = model else {
                return Ok(None);
            };
            return Ok(Some(Arc::new(OpenAiCompatibleProvider::new(
                connection.base_url,
                model,
                None,
            )?)));
        }
        if row.provider == "anthropic" {
            // A hosted assignment needs the session's key, exactly as the Cloud mode does. Without
            // one there is nothing to build, so it falls back to the configured mode instead.
            let Some(key) = self.cloud_key.clone() else {
                return Ok(None);
            };
            let model = row
                .model
                .clone()
                .filter(|model| !model.trim().is_empty())
                .unwrap_or_else(|| default_model("anthropic"));
            return Ok(Some(Arc::new(CloudProvider::new(key.expose(), model)?)));
        }
        let Some(server) = assigned_local_kind(&row.provider) else {
            return Ok(None);
        };
        let catalog = pfl_workflow::local_discovery::discover().await;
        let Some(server) = catalog.servers.iter().find(|s| s.kind == server) else {
            return Ok(None);
        };
        // An empty model means "whatever is loaded"; a named one is used as given.
        let model = match row.model.clone() {
            Some(model) if !model.trim().is_empty() => model,
            _ => match catalog.pick_loaded() {
                Some((_, model)) => model.id.clone(),
                None => return Ok(None),
            },
        };
        Ok(Some(Arc::new(OpenAiCompatibleProvider::new(
            server.base_url.clone(),
            model,
            None,
        )?)))
    }
    async fn build_provider(&self, mode: &Mode) -> Result<Arc<dyn Provider>> {
        Ok(match mode {
            Mode::Replay => self.replay.clone(),
            Mode::Local { endpoint, model } => {
                // "Local" means a runtime on this machine, not one brand of it: LM Studio,
                // llama.cpp, vLLM and Ollama's own OpenAI endpoint all answer /v1/models, while
                // Ollama answers /api/tags. Ask the server instead of guessing.
                if pfl_workflow::providers::local_flavour(endpoint).await == "openai" {
                    Arc::new(OpenAiCompatibleProvider::new(
                        endpoint.clone(),
                        model.clone(),
                        None,
                    )?)
                } else {
                    Arc::new(OllamaProvider::new(endpoint.clone(), model.clone())?)
                }
            }
            Mode::Cloud { model } => {
                let key = self.cloud_key.clone().ok_or_else(|| {
                    Error::Invalid(
                        "Cloud needs an API key for this session before a run can start".into(),
                    )
                })?;
                match std::env::var("PFL_CLOUD_ENDPOINT") {
                    Ok(endpoint) if !endpoint.is_empty() => {
                        if !is_loopback_endpoint(&endpoint) {
                            return Err(Error::Invalid(
                                "PFL_CLOUD_ENDPOINT is for loopback verification only".into(),
                            ));
                        }
                        Arc::new(CloudProvider::with_endpoint(
                            key.expose(),
                            model.clone(),
                            endpoint,
                        )?)
                    }
                    _ => Arc::new(CloudProvider::new(key.expose(), model.clone())?),
                }
            }
        })
    }

    async fn handle(&mut self, request: Request) -> Result<()> {
        match request {
            Request::Refresh => self.reply(Reply::Projects(self.service.projects().await?)),
            Request::Create(intake) => {
                let project = self.service.create_project(intake).await?;
                self.cursor = None;
                self.reply(Reply::Opened(project, Vec::new()));
                self.reply(Reply::Projects(self.service.projects().await?));
            }
            Request::Open(id) => {
                let project = self.service.project(id.clone()).await?;
                let history = self.service.history(id).await?;
                let latest = history.iter().max_by_key(|r| r.version).cloned();
                self.cursor = None;
                self.reply(Reply::Opened(project, history));
                if let Some(snapshot) = latest {
                    self.selected(snapshot).await;
                }
            }
            Request::Save(id, intake) => {
                self.reply(Reply::Saved(self.service.save_intake(id, intake).await?));
                self.reply(Reply::Projects(self.service.projects().await?));
            }
            Request::SelectRun(id) => {
                let snapshot = self.service.run(id).await?;
                self.selected(snapshot).await;
            }
            Request::ConfigureCloud { model, key } => {
                // The guard runs first: a request that will be refused must not install a key
                // into the session.
                self.ensure_idle("changing execution mode").await?;
                self.cloud_key = Some(key).filter(|value| !value.is_blank());
                let mode = Mode::Cloud { model };
                let provider = self.build_provider(&mode).await?;
                self.rebuild_service(provider);
                self.mode = Some(mode.clone());
                self.reply(Reply::Configured(mode));
            }
            Request::DiscoverProviders => {
                // An installed-but-stopped Ollama is started here, and only here: a server that
                // answers is left alone, and one this app starts is never stopped by it.
                let ollama_up = || {
                    std::net::TcpStream::connect_timeout(
                        &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
                        std::time::Duration::from_millis(250),
                    )
                    .is_ok()
                };
                let _ = pfl_workflow::ollama_supervision::ensure(
                    ollama_up,
                    || {
                        std::process::Command::new("ollama")
                            .arg("serve")
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                            .is_ok()
                    },
                    10,
                    std::time::Duration::from_millis(500),
                )
                .await;
                let env = pfl_workflow::registry::Env {
                    has_claude: pfl_workflow::cli_login::probe("claude")
                        .await
                        .unwrap_or(false),
                    has_codex: pfl_workflow::cli_login::probe("codex")
                        .await
                        .unwrap_or(false),
                    has_gemini: pfl_workflow::cli_login::probe("gemini")
                        .await
                        .unwrap_or(false),
                    has_anthropic_key: self.cloud_key.is_some(),
                    ..Default::default()
                };
                let discovered = pfl_workflow::local_discovery::discover().await;
                let sign_ins = [
                    ("claude", env.has_claude),
                    ("codex", env.has_codex),
                    ("gemini", env.has_gemini),
                ];
                let providers = pfl_workflow::catalog::build(&env, &discovered, &sign_ins);
                self.reply(Reply::Providers(providers));
            }
            Request::ListAssignments { project } => {
                let rows = self.store.assignments(&project)?;
                self.reply(Reply::Assignments(rows));
            }
            Request::SetAssignment {
                project,
                surface,
                rank,
                provider,
                model,
            } => {
                if provider.trim().is_empty() {
                    let removed = self.store.clear_assignments(&project, &surface)?;
                    let rows = self.store.assignments(&project)?;
                    let _ = removed;
                    self.reply(Reply::Assignments(rows));
                } else {
                    self.store.set_assignment(
                        &project,
                        &ModelAssignment {
                            surface: surface.clone(),
                            rank,
                            provider: provider.clone(),
                            model: model.clone(),
                            effort: None,
                        },
                    )?;
                    let rows = self.store.assignments(&project)?;
                    self.reply(Reply::Assignments(rows));
                }
            }
            Request::ClearAssignments { project, surface } => {
                let _ = self.store.clear_assignments(&project, &surface)?;
                let rows = self.store.assignments(&project)?;
                self.reply(Reply::Assignments(rows));
            }
            Request::ListLocalModels { endpoint } => {
                // Ollama first; if it does not answer, read the OpenAI-style catalogue so LM
                // Studio, llama.cpp and vLLM are listed too.
                let models = match pfl_workflow::list_local_models(&endpoint).await {
                    Ok(models) => models,
                    Err(_) => pfl_workflow::providers::list_openai_models(&endpoint).await?,
                };
                self.reply(Reply::LocalModels(models));
            }
            Request::RegenerateFrom {
                run,
                stage,
                authorization,
            } => {
                self.ensure_idle("regenerating").await?;
                let regenerated = self
                    .service
                    .regenerate_from(run, stage, authorization)
                    .await?;
                self.started_runs.insert(regenerated.id.clone());
                self.selected(regenerated).await;
            }
            Request::ExportPackage {
                project,
                claims,
                application,
                dir,
            } => {
                let claims_revision = match claims {
                    Some(id) => Some(self.service.revision(id).await?),
                    None => self
                        .service
                        .revisions(project.clone(), DocumentKind::Claims)
                        .await?
                        .pop(),
                };
                let application_revision = match application {
                    Some(id) => Some(self.service.revision(id).await?),
                    None => self
                        .service
                        .revisions(project.clone(), DocumentKind::Application)
                        .await?
                        .pop(),
                };
                let history = self.service.history(project.clone()).await?;
                let references = history
                    .iter()
                    // Only a completed feasibility run is evidence: a newer cancelled run must
                    // not replace the analysis the exported revisions were derived from.
                    .filter(|run| {
                        run.input.kind == WorkflowKind::Feasibility
                            && run.outcome == RunOutcome::Completed
                    })
                    .max_by_key(|run| run.version)
                    .map(|run| run.input.upstream.prior_art.clone())
                    .unwrap_or_default();
                let findings = pfl_workflow::package::matching_findings(
                    &history,
                    claims_revision.as_ref(),
                    application_revision.as_ref(),
                );
                let project_record = self.service.project(project).await?;
                let files = pfl_workflow::package::build_package(
                    &project_record.intake.title,
                    claims_revision.as_ref(),
                    application_revision.as_ref(),
                    &references,
                    findings,
                )?;
                pfl_workflow::package::write_package(&dir, &files)?;
                self.reply(Reply::PackageExported(dir));
            }
            Request::StartSelected {
                project,
                kind,
                idempotency_key,
                authorization,
                claims_revision,
                specification_revision,
            } => {
                // A duplicate of a run already started is idempotent, not a second run:
                // replay it instead of reporting the active-work conflict.
                if let Some(existing) = self
                    .service
                    .history(project.clone())
                    .await?
                    .into_iter()
                    .find(|run| run.idempotency_key == idempotency_key)
                {
                    self.started_runs.insert(existing.id.clone());
                    self.selected(existing).await;
                    return Ok(());
                }
                self.ensure_idle("starting another run").await?;
                for id in &self.started_runs {
                    if !self.service.run(id.clone()).await?.outcome.is_terminal() {
                        return Err(Error::Conflict(
                            "Finish or cancel active work before starting another run".into(),
                        ));
                    }
                }
                let run = self
                    .service
                    .start_workflow_with_selection(
                        project,
                        kind,
                        idempotency_key,
                        authorization,
                        Vec::new(),
                        claims_revision,
                        specification_revision,
                    )
                    .await?;
                self.started_runs.insert(run.id.clone());
                // Subscribe the new run, exactly as the plain start path does, or no
                // progress updates arrive and the run looks stuck.
                self.selected(run).await;
            }
            Request::SaveRevision {
                project,
                kind,
                text,
                note,
            } => {
                if text.trim().is_empty() {
                    return Err(Error::Invalid("an edited document needs text".into()));
                }
                let history = self.service.history(project.clone()).await?;
                let base = history
                    .iter()
                    .filter(|run| {
                        run.input.kind == kind.workflow() && run.outcome == RunOutcome::Completed
                    })
                    .max_by_key(|run| run.version)
                    .map(|run| run.id.clone())
                    .ok_or_else(|| {
                        Error::Invalid(
                            "save a completed run of this document before editing it".into(),
                        )
                    })?;
                let revision = self
                    .service
                    .save_revision(project, kind, Some(base), text, note)
                    .await?;
                self.reply(Reply::Revision(revision));
            }
            Request::ListRevisions { project, kind } => {
                let revisions = self.service.revisions(project, kind).await?;
                self.reply(Reply::Revisions(revisions));
            }
            Request::ConfigureResearch { api_key } => {
                self.ensure_idle("changing research settings").await?;
                if api_key.is_blank() {
                    return Err(Error::Invalid(
                        "research needs a PatentsView API key".into(),
                    ));
                }
                // Loopback override exists only so acceptance can count requests
                // against a mock; anything else is refused.
                let source: Arc<dyn ResearchSource> = match std::env::var("PFL_RESEARCH_ENDPOINT") {
                    Ok(endpoint) if !endpoint.is_empty() => {
                        if !is_loopback_endpoint(&endpoint) {
                            return Err(Error::Invalid(
                                "PFL_RESEARCH_ENDPOINT is for loopback verification only".into(),
                            ));
                        }
                        Arc::new(PatentsViewSource::with_endpoint(
                            api_key.expose(),
                            endpoint,
                        )?)
                    }
                    _ => Arc::new(PatentsViewSource::new(api_key.expose())?),
                };
                let provider = match &self.mode {
                    Some(mode) => self.build_provider(mode).await?,
                    None => self.replay.clone(),
                };
                self.research = Some(source);
                self.rebuild_service(provider);
                self.research_configured = true;
                self.reply(Reply::ResearchConfigured);
            }
            Request::SetCloudKey(key) => {
                // In memory only. Never logged, stored or echoed back.
                self.cloud_key = key.filter(|value| !value.is_blank());
                if self.cloud_key.is_none() && matches!(self.mode, Some(Mode::Cloud { .. })) {
                    // Clearing the key drops the Cloud provider: leaving it built would keep
                    // the key alive after the operator asked for it to go.
                    self.rebuild_service(self.replay.clone());
                    self.mode = None;
                }
                self.reply(Reply::Settings(self.service.settings().await?));
            }
            Request::SetProfile(profile) => {
                let mut settings = self.service.settings().await?;
                settings.profile = profile;
                settings.profile_chosen = true;
                self.service.save_settings(settings.clone()).await?;
                self.reply(Reply::Settings(settings));
            }
            Request::Configure(mode) => {
                self.ensure_idle("changing execution mode").await?;
                let provider = self.build_provider(&mode).await?;
                self.rebuild_service(provider);
                self.mode = Some(mode.clone());
                self.reply(Reply::Configured(mode));
            }
            Request::Start {
                project,
                kind,
                idempotency_key,
                authorization,
            } => {
                // A duplicate of a run already started is idempotent, not a second run:
                // replay it instead of reporting the active-work conflict.
                if let Some(existing) = self
                    .service
                    .history(project.clone())
                    .await?
                    .into_iter()
                    .find(|run| run.idempotency_key == idempotency_key)
                {
                    self.started_runs.insert(existing.id.clone());
                    self.selected(existing).await;
                    return Ok(());
                }
                self.ensure_idle("starting another run").await?;
                let mode = self.mode.as_ref().ok_or_else(|| {
                    Error::Invalid(
                        "Choose recorded replay or configure local inference first".into(),
                    )
                })?;
                match mode {
                    Mode::Replay if !authorization.synthetic_data => {
                        return Err(Error::Invalid(
                            "Replay requires explicitly marked synthetic data".into(),
                        ));
                    }
                    Mode::Local { .. } if !authorization.local_inference => {
                        return Err(Error::Invalid(
                            "Authorize the local inference request before starting".into(),
                        ));
                    }
                    Mode::Cloud { .. } if !authorization.paid_provider => {
                        return Err(Error::Invalid(
                            "Approve the paid Cloud request for this run before starting".into(),
                        ));
                    }
                    _ => {}
                }
                // An assignment overrides the configured mode, but only where this build can honour
                // it safely (loopback servers). Everything else keeps the mode above.
                // Per-stage providers are built before the run: the resolver is synchronous, but
                // building a provider can need discovery. A stage whose provider cannot be built
                // falls back to the workflow's.
                if let Some(workflow) = self.assigned_provider(&project, kind).await? {
                    let rows = self.store.assignments(&project)?;
                    let mut by_stage: Vec<(Stage, Arc<dyn Provider>)> = Vec::new();
                    for row in rows.iter().filter(|row| row.rank == 0) {
                        let Some(stage) = Stage::ALL
                            .iter()
                            .copied()
                            .find(|stage| row.surface == format!("stage-{}", stage.slug()))
                        else {
                            continue;
                        };
                        if let Some(provider) = self.provider_for_assignment(row).await? {
                            by_stage.push((stage, provider));
                        }
                    }
                    // The plan lists every provider the resolver can return, or a later resume is
                    // correctly refused for naming one the run never permitted.
                    let mut plan = vec![workflow.snapshot()];
                    for (_, provider) in &by_stage {
                        let snapshot = provider.snapshot();
                        if !plan.contains(&snapshot) {
                            plan.push(snapshot);
                        }
                    }
                    let fallback = workflow.clone();
                    let mut service = WorkflowService::new_with_stage_provider(
                        self.store.clone(),
                        workflow,
                        tokio::runtime::Handle::current(),
                        std::sync::Arc::new(move |stage: Stage| {
                            by_stage
                                .iter()
                                .find(|(candidate, _)| *candidate == stage)
                                .map(|(_, provider)| provider.clone())
                                .unwrap_or_else(|| fallback.clone())
                        }),
                    );
                    std::sync::Arc::get_mut(&mut service)
                        .expect("a service that was just built is not shared yet")
                        .set_provider_plan(plan);
                    self.service = service;
                }
                let snapshot = self
                    .service
                    .start_workflow(project, kind, idempotency_key, authorization)
                    .await?;
                self.started_runs.insert(snapshot.id.clone());
                self.selected(snapshot).await;
            }
            Request::Resume {
                run: id,
                authorization,
            } => {
                let snapshot = self.service.run(id.clone()).await?;
                // Resume never changes the snapshotted provider, but the authorization for
                if self.mode.is_none() {
                    return Err(Error::Invalid(
                        "Select the matching execution mode before resume".into(),
                    ));
                }
                match (&self.mode, snapshot.input.provider.kind) {
                    (Some(Mode::Replay), ProviderKind::Replay) => {}
                    (Some(Mode::Local { endpoint, model }), ProviderKind::Local)
                        if Some(endpoint) == snapshot.input.provider.endpoint.as_ref()
                            && model == &snapshot.input.provider.model => {}
                    (Some(Mode::Cloud { model }), ProviderKind::Cloud)
                        if model == &snapshot.input.provider.model => {}
                    _ => {
                        return Err(Error::Invalid(
                            "Resume must use the original provider, endpoint and model".into(),
                        ));
                    }
                }
                let snapshot = self.service.resume(id.clone(), authorization).await?;
                self.started_runs.insert(id);
                self.selected(snapshot).await;
            }
            Request::Cancel(id) => {
                let snapshot = self.service.cancel(id).await?;
                self.reply(Reply::Run(snapshot));
            }
            Request::Export { run, path, format } => {
                let snapshot = self.service.run(run).await?;
                let contents = match format {
                    ExportFormat::Markdown => export_markdown(&snapshot)?.into_bytes(),
                    ExportFormat::Html => export_html(&snapshot)?.into_bytes(),
                    ExportFormat::Word => pfl_workflow::export_docx(&snapshot)?,
                };
                let destination = path.clone();
                tokio::task::spawn_blocking(move || -> Result<()> {
                    use std::io::Write;
                    // Never silently overwrite a file, including a legacy export.
                    let mut file = std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&destination)
                        .map_err(|e| Error::Io(e.to_string()))?;
                    file.write_all(&contents)
                        .and_then(|_| file.sync_all())
                        .map_err(|e| Error::Io(e.to_string()))?;
                    Ok(())
                })
                .await
                .map_err(|e| Error::Io(e.to_string()))??;
                self.reply(Reply::Exported(path));
            }
            Request::Shutdown => {}
        }
        Ok(())
    }

    /// Keep the app-wide active-job count truthful, including jobs owned by a
    /// project or a historical run that the current view no longer selects.
    async fn refresh_active(&mut self) {
        let ids: Vec<RunId> = self.started_runs.iter().cloned().collect();
        let mut live = 0usize;
        let mut finished = Vec::new();
        for id in ids {
            match self.service.run(id.clone()).await {
                Ok(snapshot) if !snapshot.outcome.is_terminal() => live += 1,
                _ => finished.push(id),
            }
        }
        for id in finished {
            self.started_runs.remove(&id);
        }
        self.active_jobs.store(live, Ordering::Release);
    }

    async fn poll(&mut self) -> Result<()> {
        let Some(cursor) = self.cursor.clone() else {
            return Ok(());
        };
        let events = self.service.events(cursor.clone()).await?;
        // Never acknowledge gaps, even if an adapter violates the durable API.
        let mut after = cursor.after_sequence;
        for event in &events {
            if event.run_id != cursor.run_id || event.sequence != after + 1 {
                return Err(Error::Conflict(
                    "Event sequence gap; reopen this run to replay safely".into(),
                ));
            }
            after = event.sequence;
        }
        let snapshot = self.service.run(cursor.run_id.clone()).await?;
        self.cursor = Some(Subscribe {
            after_sequence: after,
            ..cursor
        });
        if events.is_empty() && snapshot.outcome.is_terminal() {
            return Ok(());
        }
        self.reply(Reply::Poll { events, snapshot });
        Ok(())
    }
}

/// Product-only state, never a legacy installation or a working-directory DB.
pub fn data_directory() -> Result<PathBuf> {
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/share")));
    let base = base.filter(|p| p.is_absolute()).ok_or_else(|| {
        Error::Io("No absolute per-user application data location is available".into())
    })?;
    Ok(base.join("patent-forge-lite"))
}
/// Which local server an assignment names, if it names one this build may use.
///
/// `None` is deliberate for every off-machine provider: a run must not reach a third party until
/// the consent matrix covers that kind, and returning `None` keeps the configured mode instead.
/// A subscription program an assignment may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssignedCli {
    Claude,
    Codex,
    Gemini,
}

/// Which signed-in program an assignment names, if it names one.
///
/// The consent rule in the workflow covers these kinds, so they are safe to build: a run carrying
/// real-invention data is refused before the program is ever started.
pub(crate) fn assigned_cli_kind(slug: &str) -> Option<AssignedCli> {
    match slug {
        "claude" => Some(AssignedCli::Claude),
        "codex" => Some(AssignedCli::Codex),
        "gemini" => Some(AssignedCli::Gemini),
        _ => None,
    }
}

/// The model a registry entry names, used when the assignment names none.
fn default_model(slug: &str) -> String {
    pfl_workflow::registry::PROVIDER_REGISTRY
        .iter()
        .find(|entry| entry.id.slug() == slug)
        .map(|entry| entry.model.to_string())
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| slug.to_string())
}
pub(crate) fn assigned_local_kind(slug: &str) -> Option<pfl_workflow::local_discovery::LocalKind> {
    use pfl_workflow::local_discovery::LocalKind;
    match slug {
        "local-lmstudio" => Some(LocalKind::LmStudio),
        "local-ollama" => Some(LocalKind::Ollama),
        "local-llamacpp" => Some(LocalKind::LlamaCpp),
        _ => None,
    }
}

#[cfg(test)]
mod assigned_tests {
    use super::*;

    #[test]
    fn only_loopback_providers_are_accepted_from_an_assignment_yet() {
        assert!(assigned_local_kind("local-ollama").is_some());
        assert!(assigned_local_kind("local-lmstudio").is_some());
        assert!(assigned_local_kind("local-llamacpp").is_some());
        for off_machine in ["claude", "codex", "gemini", "anthropic", "openai"] {
            assert!(
                assigned_local_kind(off_machine).is_none(),
                "{off_machine} must not be used from an assignment until consent covers it"
            );
        }
        assert!(assigned_local_kind("nonsense").is_none());
    }
}
