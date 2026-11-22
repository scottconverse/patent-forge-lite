//! App-lifetime async bridge. Only explicit requests can create or resume jobs.
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::{
    CloudProvider, OllamaProvider, ReplayProvider, WorkflowService, export_html, export_markdown,
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

#[derive(Debug)]
pub enum Request {
    Refresh,
    Create(Intake),
    Open(ProjectId),
    Save(ProjectId, Intake),
    SelectRun(RunId),
    Configure(Mode),
    /// Holds the Cloud key in memory for this session only. A `None` clears it.
    /// The value is never stored, logged, exported or echoed back in a reply.
    SetCloudKey(Option<String>),
    SetProfile(SetupProfile),
    Start {
        project: ProjectId,
        kind: WorkflowKind,
        idempotency_key: String,
        authorization: Authorization,
    },
    Resume(RunId),
    Cancel(RunId),
    Export {
        run: RunId,
        path: PathBuf,
        format: ExportFormat,
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
    Recovered(Vec<RunId>),
    Error(String),
    Stopped,
}

/// The runtime and runner outlive individual GPUI windows and navigation.
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
            Error::Conflict("Patent Forge Lite is already using this data location".into())
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
                cloud_key: None,
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
    /// In-memory only, for this session. Never persisted, never logged.
    cloud_key: Option<String>,
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
            Request::SetCloudKey(key) => {
                // In memory only. Never logged, stored or echoed back.
                self.cloud_key = key.filter(|value| !value.trim().is_empty());
                self.reply(Reply::Settings(self.service.settings().await?));
            }
            Request::SetProfile(profile) => {
                let mut settings = self.service.settings().await?;
                settings.profile = profile;
                self.service.save_settings(settings.clone()).await?;
                self.reply(Reply::Settings(settings));
            }
            Request::Configure(mode) => {
                for id in &self.started_runs {
                    if !self.service.run(id.clone()).await?.outcome.is_terminal() {
                        return Err(Error::Conflict(
                            "Finish or cancel active work before changing execution mode".into(),
                        ));
                    }
                }
                let provider: Arc<dyn Provider> = match &mode {
                    Mode::Replay => self.replay.clone(),
                    Mode::Local { endpoint, model } => {
                        Arc::new(OllamaProvider::new(endpoint.clone(), model.clone())?)
                    }
                    Mode::Cloud { model } => {
                        let key = self.cloud_key.clone().ok_or_else(|| {
                            Error::Invalid(
                                "Cloud needs an API key for this session before a run can start"
                                    .into(),
                            )
                        })?;
                        Arc::new(CloudProvider::new(key, model.clone())?)
                    }
                };
                self.service = WorkflowService::new(
                    self.store.clone(),
                    provider,
                    tokio::runtime::Handle::current(),
                );
                self.mode = Some(mode.clone());
                self.reply(Reply::Configured(mode));
            }
            Request::Start {
                project,
                kind,
                idempotency_key,
                authorization,
            } => {
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
                let snapshot = self
                    .service
                    .start_workflow(project, kind, idempotency_key, authorization)
                    .await?;
                self.started_runs.insert(snapshot.id.clone());
                self.selected(snapshot).await;
            }
            Request::Resume(id) => {
                let snapshot = self.service.run(id.clone()).await?;
                // Resume never changes the snapshotted provider or authorizations.
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
                let snapshot = self.service.resume(id.clone()).await?;
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
