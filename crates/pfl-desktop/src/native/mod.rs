//! GPUI desktop. Provider execution is exclusively owned by WorkflowService.
mod assets;
mod input;
mod theme;

use crate::{
    editor::TextBuffer,
    session::{
        APPLICATION_ID, APPLICATION_NAME, ApplicationSession, ExportFormat, Mode, Reply, Request,
        data_directory,
    },
    state::{DesktopState, EventApplied, Page, context_from_values},
};
use gpui::{
    App, Bounds, BoxShadow, Context, Div, Entity, FocusHandle, FontWeight, KeyBinding, Role,
    SharedString, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, rgba,
    size,
};
use input::TextInput;
use pfl_domain::*;
use std::{rc::Rc, time::Duration};

actions!(
    pfl_desktop,
    [
        SaveIntake,
        FocusNext,
        FocusPrevious,
        Quit,
        NavProjects,
        NavIntake,
        NavAnalysis,
        NavHistory,
        NavReport,
        NavSettings,
        StartRun
    ]
);

/// Parsed launch options. They only choose a starting view; nothing here can
/// start a run, contact a provider or create a project.
struct Launch {
    replay: bool,
    page: Option<Page>,
    select_latest: bool,
    local_endpoint: Option<String>,
}

fn parse_args() -> Result<Option<Launch>> {
    let mut launch = Launch {
        replay: false,
        page: None,
        select_latest: false,
        local_endpoint: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--replay" => launch.replay = true,
            "--select-latest" => launch.select_latest = true,
            // Loopback only: lets acceptance point the app at a stand-in runtime without
            // typing into the endpoint field.
            "--local-endpoint" => {
                let value = args
                    .next()
                    .ok_or_else(|| Error::Invalid("--local-endpoint requires a URL".into()))?;
                let loopback = ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
                    .iter()
                    .any(|prefix| value.starts_with(prefix));
                if !loopback {
                    return Err(Error::Invalid(
                        "--local-endpoint is for a loopback runtime only".into(),
                    ));
                }
                launch.local_endpoint = Some(value);
            }
            "--page" => {
                let value = args.next().ok_or_else(|| {
                    Error::Invalid("--page requires one of: projects, intake, analysis, history, report, setup".into())
                })?;
                launch.page = Some(match value.as_str() {
                    "projects" => Page::Projects,
                    "intake" => Page::Intake,
                    "analysis" | "workflow" => Page::Workflow,
                    "history" => Page::History,
                    "report" => Page::Report,
                    "setup" | "settings" => Page::Settings,
                    other => {
                        return Err(Error::Invalid(format!(
                            "unknown page {other}; use projects, intake, analysis, history, report or setup"
                        )));
                    }
                });
            }
            "--help" | "-h" => {
                println!(
                    "Patent Forge Lite\n  --replay         select synthetic recorded responses (does not start a run)\n  --page <name>    open directly to projects|intake|analysis|history|report|setup\n  --select-latest  select the most recent project and its latest saved run
  --local-endpoint <url>  loopback runtime to prefill the endpoint field (acceptance)\n\nKeyboard: Ctrl+1..6 pages, Ctrl+S save, Ctrl+Q quit."
                );
                return Ok(None);
            }
            other => {
                return Err(Error::Invalid(format!(
                    "unknown option {other}; supported: --replay, --page <name>, --select-latest, --help"
                )));
            }
        }
    }
    Ok(Some(launch))
}

pub fn run() -> Result<()> {
    let Some(launch) = parse_args()? else {
        return Ok(());
    };
    let replay = launch.replay;
    let initial_page = launch.page;
    let select_latest = launch.select_latest;
    let directory = data_directory()?;
    std::fs::create_dir_all(&directory).map_err(|e| Error::Io(e.to_string()))?;
    let session = Rc::new(ApplicationSession::open(
        directory.join("projects.sqlite3"),
    )?);
    gpui_platform::application()
        .with_assets(assets::BrandAssets)
        .run(move |cx: &mut App| {
        let _ = cx.text_system().add_fonts(assets::font_blobs());
        cx.set_app_identity(APPLICATION_ID, APPLICATION_NAME);
        input::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrevious, None),
            // Direct page navigation, so every screen is reachable by keyboard.
            KeyBinding::new("ctrl-1", NavProjects, None),
            KeyBinding::new("ctrl-2", NavIntake, None),
            KeyBinding::new("ctrl-3", NavAnalysis, None),
        // Keyboard path to start the current workflow, so the action is reachable
        // without a pointer and without depending on an automation invoke.
        KeyBinding::new("ctrl-enter", StartRun, None),
            KeyBinding::new("ctrl-4", NavHistory, None),
            KeyBinding::new("ctrl-5", NavReport, None),
            KeyBinding::new("ctrl-6", NavSettings, None),
        ]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([KeyBinding::new("cmd-s", SaveIntake, None), KeyBinding::new("cmd-q", Quit, None)]);
        #[cfg(not(target_os = "macos"))]
        cx.bind_keys([KeyBinding::new("ctrl-s", SaveIntake, None), KeyBinding::new("ctrl-q", Quit, None)]);

        let quitting = session.clone();
        cx.on_app_quit(move |cx| {
            let session = quitting.clone();
            let executor = cx.background_executor().clone();
            let _ = session.request(Request::Shutdown);
            async move {
                // Workflow cancellation/checkpointing, not runtime process shutdown.
                for _ in 0..200 {
                    if session.is_stopped() { break; }
                    executor.timer(Duration::from_millis(50)).await;
                }
            }
        }).detach();
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() { cx.quit(); }
        }).detach();
        let bounds = Bounds::centered(None, size(px(1120.), px(820.)), cx);
        let result = cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |window, cx| {
                window.set_app_id(APPLICATION_ID);
                window.set_window_title(APPLICATION_NAME);
                cx.new(|cx| {
                    Desktop::new(
                        session.clone(),
                        replay,
                        initial_page,
                        select_latest,
                        launch.local_endpoint,
                        window,
                        cx,
                    )
                })
            },
        );
        match result {
            Ok(handle) => {
                cx.activate(true);
                let _ = handle.update(cx, |desktop, window, cx| {
                    // Focus arrives after the first frame: the window is not yet
                    // focusable while it is being created, and GPUI only routes
                    // key bindings through a focused element.
                    let focus = desktop.content_focus.clone();
                    window.on_next_frame(move |window, cx| {
                        window.focus(&focus, cx);
                    });
                    cx.notify();
                });
            }
            Err(error) => {
                eprintln!("Patent Forge Lite could not open its native window: {error}. Check the supported graphics driver and desktop-session requirements.");
                let _ = session.request(Request::Shutdown);
                cx.quit();
            }
        }
    });
    Ok(())
}

struct Desktop {
    session: Rc<ApplicationSession>,
    state: DesktopState,
    title: Entity<TextInput>,
    description: Entity<TextInput>,
    field: Entity<TextInput>,
    context: [Entity<TextInput>; 9],
    advanced: bool,
    report: Entity<TextInput>,
    stream: Entity<TextInput>,
    endpoint: Entity<TextInput>,
    /// The name and endpoint of a connection being added. Records only; nothing is contacted.
    connection_name: Entity<TextInput>,
    connection_url: Entity<TextInput>,
    model: Entity<TextInput>,
    saved_intake: Intake,
    mode: Option<Mode>,
    workflow: WorkflowKind,
    profile: SetupProfile,
    synthetic: bool,
    /// The operator acknowledged the invention is real and stays on this machine.
    real_data: bool,
    authorized_local: bool,
    /// The operator approved the paid Cloud request for the next run.
    cloud_approved: bool,
    cloud_model: Entity<TextInput>,
    cloud_key: Entity<TextInput>,
    research_key: Entity<TextInput>,
    research_authorized: bool,
    research_configured: bool,
    /// True until the operator has chosen Full or Lean on a fresh profile.
    first_run: bool,
    /// Models the local runtime reported, newest listing wins.
    local_models: Vec<String>,
    /// The joined provider catalogue, refreshed on request.
    providers: Vec<pfl_workflow::catalog::ProviderView>,
    /// Which provider runs each surface for the open project.
    assignments: Vec<pfl_domain::ModelAssignment>,
    /// Saved custom endpoints, which a run may address as `conn-<id>`.
    connections: Vec<pfl_domain::ApiConnection>,
    /// Honest readiness text for the runtime: a count, or why it could not be asked.
    local_status: String,
    revisions: Vec<DocumentRevision>,
    /// The last comparison shown, rendered from two stored revisions.
    revision_diff: Option<pfl_workflow::diff::RevisionDiff>,
    /// Revisions a compliance run will review when it is started from the window.
    compliance_claims: Option<RevisionId>,
    compliance_specification: Option<RevisionId>,
    revision_text: Entity<TextInput>,
    pending: bool,
    start_key: Option<String>,
    pending_open: Option<ProjectId>,
    confirm_close: bool,
    allow_close: bool,
    /// Set by `--select-latest`: open the newest project once the list arrives.
    auto_open_latest: bool,
    /// Applied once a run is selected, so `--page report` lands on real content.
    auto_page: Option<Page>,
    /// Rendered document vs raw Markdown source for the report pane.
    report_rendered: bool,
    /// Focus target for the page body. GPUI dispatches key bindings through the
    /// focused element, so the window must start with real focus or global
    /// shortcuts and Tab traversal never fire.
    content_focus: FocusHandle,
}

/// The platform's display name, from the target the binary was built for.
fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}
impl Desktop {
    fn new(
        session: Rc<ApplicationSession>,
        replay: bool,
        initial_page: Option<Page>,
        select_latest: bool,
        launched_endpoint: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Invention title")
        });
        let description = cx
            .new(|cx| TextInput::new(TextBuffer::new("", true), 250., cx).labelled("Description"));
        let field = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Technical field")
        });
        let context = std::array::from_fn(|_| {
            cx.new(|cx| {
                TextInput::new(TextBuffer::new("", true), 110., cx)
                    .labelled("Optional context field")
            })
        });
        let report = cx.new(|cx| {
            TextInput::new(TextBuffer::read_only("Select a saved report."), 500., cx)
                .labelled("Saved report")
        });
        let stream = cx.new(|cx| {
            TextInput::new(TextBuffer::read_only("No analysis running."), 390., cx)
                .labelled("Stage output")
        });
        let endpoint_hint = launched_endpoint
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:11434".into());
        let endpoint = cx.new(|cx| {
            TextInput::new(TextBuffer::new(endpoint_hint, false), 42., cx)
                .labelled("Local endpoint")
        });
        let connection_name = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Connection name")
        });
        let connection_url =
            cx.new(|cx| TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Base URL"));
        let model = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Local model name")
        });
        let cloud_model = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx).labelled("Cloud model name")
        });
        let revision_text = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", true), 160., cx).labelled("Revision text")
        });
        let research_key = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx)
                .labelled("PatentsView API key")
                .masked(true)
        });
        let cloud_key = cx.new(|cx| {
            TextInput::new(TextBuffer::new("", false), 42., cx)
                .labelled("Cloud API key")
                .masked(true)
        });
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |_, cx| {
            view.update(cx, |view, cx| view.can_close(cx))
                .unwrap_or(true)
        });
        if replay {
            let _ = session.request(Request::Configure(Mode::Replay));
        }
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(90))
                    .await;
                if view.update(cx, |view, cx| view.receive(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
        Self {
            session,
            state: DesktopState::default(),
            title,
            description,
            field,
            context,
            advanced: false,
            report,
            stream,
            endpoint,
            connection_name,
            connection_url,
            model,
            saved_intake: Intake::default(),
            mode: None,
            workflow: WorkflowKind::Feasibility,
            profile: SetupProfile::Lean,
            synthetic: false,
            real_data: false,
            authorized_local: false,
            cloud_approved: false,
            cloud_model,
            cloud_key,
            research_key,
            research_authorized: false,
            research_configured: false,
            first_run: false,
            local_models: Vec::new(),
            providers: Vec::new(),
            assignments: Vec::new(),
            connections: Vec::new(),
            local_status: "Runtime not queried yet.".into(),
            revisions: Vec::new(),
            revision_diff: None,
            compliance_claims: None,
            compliance_specification: None,
            revision_text,
            pending: false,
            start_key: None,
            pending_open: None,
            confirm_close: false,
            allow_close: false,
            auto_open_latest: select_latest,
            auto_page: initial_page,
            report_rendered: true,
            content_focus: cx.focus_handle(),
        }
    }

    /// Restores keyboard focus to the page body after navigation, so global
    /// shortcuts keep working once the previously focused control is gone.
    fn focus_body(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.content_focus, cx);
    }

    fn send(&mut self, request: Request, cx: &mut Context<Self>) {
        if let Err(error) = self.session.request(request) {
            self.state.notice = Some(error.to_string());
        }
        cx.notify();
    }

    fn intake(&self, cx: &App) -> Intake {
        Intake {
            title: self.title.read(cx).buffer.text().into(),
            description: self.description.read(cx).buffer.text().into(),
            technical_field: self.field.read(cx).buffer.text().into(),
            context: context_from_values(std::array::from_fn(|i| {
                self.context[i].read(cx).buffer.text().to_owned()
            })),
        }
    }

    fn dirty(&self, cx: &App) -> bool {
        self.intake(cx) != self.saved_intake
    }

    fn can_close(&mut self, cx: &mut Context<Self>) -> bool {
        let selected_active = self.state.run.as_ref().is_some_and(|run| {
            matches!(
                run.snapshot.outcome,
                RunOutcome::Running | RunOutcome::Cancelling
            )
        });
        // App-owned jobs count even when another project or a historical run owns
        // them; navigation never cancels work, so quitting must still warn.
        let active = selected_active || self.session.active_jobs() > 0;
        if !self.allow_close && (self.dirty(cx) || self.pending || active) {
            self.confirm_close = true;
            cx.notify();
            return false;
        }
        true
    }

    fn quit(&mut self, cx: &mut Context<Self>) {
        if self.can_close(cx) {
            cx.quit();
        }
    }

    fn load_intake(&mut self, intake: &Intake, cx: &mut Context<Self>) {
        self.title
            .update(cx, |input, cx| input.load(intake.title.clone(), cx));
        self.description
            .update(cx, |input, cx| input.load(intake.description.clone(), cx));
        self.field.update(cx, |input, cx| {
            input.load(intake.technical_field.clone(), cx)
        });
        for (index, (_, value)) in intake.context.labeled_fields().into_iter().enumerate() {
            self.context[index].update(cx, |input, cx| input.load(value, cx));
        }
        self.saved_intake = intake.clone();
    }

    fn receive(&mut self, cx: &mut Context<Self>) {
        let replies = self.session.drain();
        if replies.is_empty() {
            return;
        }
        for reply in replies {
            match reply {
                Reply::Projects(projects) => {
                    // `--select-latest` opens the newest project once, on the first list.
                    if self.auto_open_latest
                        && let Some(project) = projects.iter().max_by_key(|p| p.updated_at_ms)
                    {
                        self.auto_open_latest = false;
                        let id = project.id.clone();
                        self.state.notice = Some("Opening the most recent project.".into());
                        self.send(Request::Open(id), cx);
                    }
                    self.state.projects = projects;
                }
                Reply::Opened(project, history) => {
                    self.load_intake(&project.intake, cx);
                    self.state.open(project);
                    self.state.history = history;
                    self.pending = false;
                    self.start_key = None;
                    self.authorized_local = false;
                }
                Reply::Saved(project) => {
                    // Do not replace editor content: typing may continue while save is in flight.
                    self.saved_intake = project.intake.clone();
                    self.state.selected_project = Some(project);
                    self.state.notice = Some("Intake saved.".into());
                    self.pending = false;
                    self.start_key = None;
                    self.authorized_local = false;
                }
                Reply::Run(snapshot) => {
                    // The selected run owns the workflow identity: showing an
                    // application run must not leave the header claiming feasibility.
                    self.workflow = snapshot.input.kind;
                    self.state.show_run(snapshot);
                    self.pending = false;
                    self.start_key = None;
                    if let Some(page) = self.auto_page.take() {
                        self.state.navigate(page);
                    }
                }
                Reply::Poll { events, snapshot } => {
                    self.state.remember_run(snapshot.clone());
                    if let Some(view) = &mut self.state.run
                        && view.snapshot.id == snapshot.id
                    {
                        for event in events {
                            if view.apply(&event) == EventApplied::Gap {
                                self.state.notice = Some(
                                    "Event gap detected. Reopen the run to replay safely.".into(),
                                );
                                break;
                            }
                        }
                        view.refresh(snapshot);
                    }
                }
                Reply::Configured(mode) => {
                    self.mode = Some(mode);
                    self.authorized_local = false;
                    self.start_key = None;
                    self.state.notice =
                        Some("Execution mode configured. No inference request was sent.".into());
                }
                // The Models page consumes this next; holding it here keeps the reply from being
                // dropped on the floor while the page is built.
                Reply::Assignments(rows) => {
                    self.assignments = rows;
                    cx.notify();
                }
                Reply::Connections(rows) => {
                    self.connections = rows;
                    cx.notify();
                }
                Reply::Providers(providers) => {
                    self.providers = providers;
                    cx.notify();
                }
                Reply::LocalModels(models) => {
                    self.local_status = if models.is_empty() {
                        "The runtime answered, but reports no installed models.".into()
                    } else {
                        format!("{} model(s) available from the runtime.", models.len())
                    };
                    self.local_models = models;
                }
                Reply::ResearchConfigured => {
                    self.state.notice =
                        Some("Research configured. It runs only for a run that opts in.".into());
                }
                Reply::PackageExported(path) => {
                    self.state.notice = Some(format!("Package written to {}", path.display()));
                }
                Reply::Revision(revision) => {
                    self.revisions.push(revision.clone());
                    self.state.notice = Some(format!(
                        "Saved revision {} of {} document.",
                        revision.version,
                        revision.kind.slug()
                    ));
                }
                Reply::Revisions(list) => {
                    self.revisions = list;
                    self.state.notice =
                        Some(format!("{} stored revision(s).", self.revisions.len()));
                }
                Reply::Exported(_) => {
                    self.state.notice = Some("Saved report exported to the selected file.".into())
                }
                Reply::Recovered(ids) if !ids.is_empty() => {
                    self.state.notice = Some(format!(
                        "Recovered {} interrupted run(s). Resume is explicit.",
                        ids.len()
                    ))
                }
                Reply::Error(error) => {
                    // A failed model listing must read as unreachable, not as an empty
                    // runtime: readiness stays honest.
                    if self.local_status.starts_with("Querying the runtime") {
                        self.local_status = format!("Runtime unreachable: {error}");
                    }
                    self.state.notice = Some(error);
                    self.pending = false;
                }
                Reply::Settings(settings) => {
                    self.profile = settings.profile;
                    self.first_run = !settings.profile_chosen;
                    cx.notify();
                }
                Reply::Stopped | Reply::Recovered(_) => {}
            }
        }
        if let Some(view) = &self.state.run {
            let markdown = view
                .snapshot
                .report
                .as_ref()
                .map(|r| r.markdown.as_str())
                .unwrap_or("This run has no saved report yet.");
            if self.report.read(cx).buffer.text() != markdown {
                self.report.update(cx, |input, cx| input.load(markdown, cx));
            }
            let mut output = String::new();
            for stage in view.snapshot.input.kind.stages() {
                let text = view.stage_text(*stage);
                if !text.is_empty() {
                    output.push_str(&format!(
                        "Stage {} · {}\n{}\n\n",
                        stage.number(),
                        stage.label(),
                        text
                    ));
                }
            }
            if output.is_empty() {
                output = "Waiting for stage output. Viewing does not start inference.".into();
            }
            if self.stream.read(cx).buffer.text() != output {
                self.stream.update(cx, |input, cx| {
                    let selection = input.buffer.selection();
                    let reversed = input.buffer.reversed();
                    input.buffer.load(output);
                    if reversed {
                        input.buffer.select(selection.end, selection.start);
                    } else {
                        input.buffer.select(selection.start, selection.end);
                    }
                    cx.notify();
                });
            }
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        // One pure decision, tested in `state::gate_tests`.
        if let Some(reason) = crate::state::save_block(self.pending) {
            self.state.notice = Some(reason.to_string());
            return;
        }
        let intake = self.intake(cx);
        self.pending = true;
        let request = match &self.state.selected_project {
            Some(project) => Request::Save(project.id.clone(), intake),
            None => Request::Create(intake),
        };
        self.send(request, cx);
    }

    /// Everything the start guard needs, in one place, so start, regenerate and resume all
    /// judge the same conditions.
    fn start_gate(&self, cx: &App) -> crate::state::StartGate {
        crate::state::StartGate {
            pending: self.pending,
            dirty: self.dirty(cx),
            has_project: self.state.selected_project.is_some(),
            run_active: self.state.run.as_ref().is_some_and(|r| {
                matches!(
                    r.snapshot.outcome,
                    RunOutcome::Running | RunOutcome::Cancelling
                )
            }),
            data_declared: self.synthetic || self.real_data,
            mode_is_local: matches!(self.mode, Some(Mode::Local { .. })),
            local_authorized: self.authorized_local,
            mode_is_cloud: matches!(self.mode, Some(Mode::Cloud { .. })),
            cloud_approved: self.cloud_approved,
        }
    }

    /// True when the action was refused, in which case the reason is now the notice.
    fn gate_blocks(&mut self, cx: &mut Context<Self>) -> bool {
        let gate = self.start_gate(cx);
        match crate::state::start_block(&gate) {
            Some(reason) => {
                self.state.notice = Some(reason.to_string());
                true
            }
            None => false,
        }
    }

    /// The authorization the operator has granted in this window, right now.
    fn authorization(&self) -> Authorization {
        Authorization {
            synthetic_data: self.synthetic,
            real_data: self.real_data,
            local_inference: self.authorized_local,
            paid_provider: self.cloud_approved,
            research: self.research_authorized,
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        // One pure decision, shared with regenerate and resume.
        if self.gate_blocks(cx) {
            return;
        }
        // The gate guarantees a project is selected; bind it for the request.
        let Some(project) = self.state.selected_project.clone() else {
            return;
        };
        let key = self
            .start_key
            .get_or_insert_with(|| RunId::new().to_string())
            .clone();
        let request = Request::Start {
            project: project.id.clone(),
            kind: self.workflow,
            idempotency_key: key,
            authorization: self.authorization(),
        };
        self.pending = true;
        self.state.page = Page::Workflow;
        self.send(request, cx);
    }

    fn export(&mut self, format: ExportFormat, cx: &mut Context<Self>) {
        let Some(view) = &self.state.run else {
            return;
        };
        if view.snapshot.report.is_none() {
            self.state.notice = Some("Choose a completed saved report before exporting.".into());
            return;
        }
        let run = view.snapshot.id.clone();
        let version = view.snapshot.version;
        let extension = match format {
            ExportFormat::Markdown => "md",
            ExportFormat::Html => "html",
            ExportFormat::Word => "docx",
        };
        let directory = data_directory().unwrap_or_default();
        let suggested = format!("patent-forge-lite-report-v{version}.{extension}");
        let prompt = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn(async move |view, cx| match prompt.await {
            Ok(Ok(Some(path))) => {
                let _ = view.update(cx, |view, cx| {
                    view.send(Request::Export { run, path, format }, cx);
                });
            }
            Ok(Ok(None)) => {}
            _ => {
                let _ = view.update(cx, |view, cx| {
                    view.state.notice = Some(
                        "Could not open the native save dialog. No export was written.".into(),
                    );
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Secondary action: bordered surface button.
    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let action = Rc::new(action);
        let keyboard_action = action.clone();
        let text: SharedString = label.into();
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(text.clone())
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(theme::SURFACE_2))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .text_color(rgb(theme::TEXT))
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .hover(|s| s.bg(rgb(0x1f2c3c)).border_color(rgb(0x36506e)))
            .focus_visible(|s| s.border_color(rgb(theme::ACCENT)).bg(rgb(0x1f2c3c)))
            .child(text)
            // The action may only set a notice. Without this repaint the notice is invisible
            // and the button reads as dead, which is the failure the notices exist to avoid.
            .on_click(cx.listener(move |this, _, window, cx| {
                action(this, window, cx);
                cx.notify();
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                        keyboard_action(this, window, cx);
                        cx.notify();
                        cx.stop_propagation();
                    }
                }),
            )
    }

    /// The control that names a provider for the next run only.
    fn provider_override_button(
        &self,
        provider: &pfl_workflow::catalog::ProviderView,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if !provider.available {
            return div().into_any_element();
        }
        let id = provider.id.clone();
        self.ghost_button(
            format!("override-{}", provider.id),
            "Next run only",
            move |this, _, cx| {
                this.send(
                    Request::SetOverride {
                        provider: Some(id.clone()),
                    },
                    cx,
                );
            },
            cx,
        )
        .into_any_element()
    }
    /// The control that names a provider for the final report alone.
    ///
    /// One stage is the honest place to start: the report is the stage an operator most often wants
    /// a different model for, and a per-stage picker for all nineteen stages would be a UI in
    /// search of a problem. The store and the run path already handle any stage surface.
    fn provider_stage_button(
        &self,
        provider: &pfl_workflow::catalog::ProviderView,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(project) = self.state.selected_project.as_ref().map(|p| p.id.clone()) else {
            return div().into_any_element();
        };
        if !provider.available {
            return div().into_any_element();
        }
        let id = provider.id.clone();
        let surface = format!("stage-{}", pfl_domain::Stage::FinalReport.slug());
        let in_use = self
            .assignments
            .iter()
            .any(|row| row.surface == surface && row.provider == provider.id && row.rank == 0);
        let label = if in_use {
            "Report only ✓"
        } else {
            "Report only"
        };
        self.ghost_button(
            format!("stage-{}", provider.id),
            label,
            move |this, _, cx| {
                this.send(
                    Request::SetAssignment {
                        project: project.clone(),
                        surface: format!("stage-{}", pfl_domain::Stage::FinalReport.slug()),
                        rank: 0,
                        provider: id.clone(),
                        model: None,
                    },
                    cx,
                );
            },
            cx,
        )
        .into_any_element()
    }
    /// The control that makes a provider the fallback when the choice fails.
    fn provider_fallback_button(
        &self,
        provider: &pfl_workflow::catalog::ProviderView,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(project) = self.state.selected_project.as_ref().map(|p| p.id.clone()) else {
            return div().into_any_element();
        };
        if !provider.available {
            return div().into_any_element();
        }
        let id = provider.id.clone();
        self.ghost_button(
            format!("fallback-{}", provider.id),
            if self
                .assignments
                .iter()
                .any(|row| row.provider == provider.id && row.rank == 1)
            {
                "Is fallback"
            } else {
                "As fallback"
            },
            move |this, _, cx| {
                for kind in pfl_domain::WorkflowKind::ALL {
                    this.send(
                        Request::SetAssignment {
                            project: project.clone(),
                            surface: format!("workflow-{}", kind.slug()),
                            rank: 1,
                            provider: id.clone(),
                            model: None,
                        },
                        cx,
                    );
                }
            },
            cx,
        )
        .into_any_element()
    }
    /// The control that makes a provider the choice for every workflow.
    ///
    /// Per-stage assignment is the finer grain the store already supports; this is the coarse
    /// "use this for everything" that an operator reaches for first.
    fn provider_choice_button(
        &self,
        provider: &pfl_workflow::catalog::ProviderView,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(project) = self.state.selected_project.as_ref().map(|p| p.id.clone()) else {
            return div()
                .text_xs()
                .text_color(rgb(theme::FAINT))
                .child("Open a project to choose a provider")
                .into_any_element();
        };
        if !provider.available {
            return div().into_any_element();
        }
        let id = provider.id.clone();
        self.button(
            format!("use-{}", provider.id),
            if self
                .assignments
                .iter()
                .any(|row| row.provider == provider.id && row.rank == 0)
            {
                "In use"
            } else {
                "Use for every workflow"
            },
            move |this, _, cx| {
                for kind in pfl_domain::WorkflowKind::ALL {
                    this.send(
                        Request::SetAssignment {
                            project: project.clone(),
                            surface: format!("workflow-{}", kind.slug()),
                            rank: 0,
                            provider: id.clone(),
                            model: None,
                        },
                        cx,
                    );
                }
            },
            cx,
        )
        .into_any_element()
    }
    /// Primary action: accent-filled.
    fn primary_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let action = Rc::new(action);
        let keyboard_action = action.clone();
        let text: SharedString = label.into();
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(text.clone())
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_md()
            .bg(rgb(theme::ACCENT))
            .text_color(rgb(0x08111d))
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .shadow(vec![
                BoxShadow::new(px(0.), px(6.), rgba(0x4d8dff40).into())
                    .blur_radius(px(18.))
                    .spread_radius(px(-6.)),
            ])
            .hover(|s| s.bg(rgb(0x6ba0ff)))
            .focus_visible(|s| s.border_color(rgb(theme::TEXT)).border_1())
            .child(text)
            // The action may only set a notice. Without this repaint the notice is invisible
            // and the button reads as dead, which is the failure the notices exist to avoid.
            .on_click(cx.listener(move |this, _, window, cx| {
                action(this, window, cx);
                cx.notify();
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                        keyboard_action(this, window, cx);
                        cx.notify();
                        cx.stop_propagation();
                    }
                }),
            )
    }

    /// Quiet action: text-only with a hover surface.
    fn ghost_button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let action = Rc::new(action);
        let keyboard_action = action.clone();
        let text: SharedString = label.into();
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .role(Role::Button)
            .aria_label(text.clone())
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .text_color(rgb(theme::MUTED))
            .text_sm()
            .hover(|s| s.bg(rgb(theme::SURFACE_2)).text_color(rgb(theme::TEXT)))
            .focus_visible(|s| {
                s.border_color(rgb(theme::ACCENT))
                    .text_color(rgb(theme::TEXT))
            })
            .child(text)
            // The action may only set a notice. Without this repaint the notice is invisible
            // and the button reads as dead, which is the failure the notices exist to avoid.
            .on_click(cx.listener(move |this, _, window, cx| {
                action(this, window, cx);
                cx.notify();
            }))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                        keyboard_action(this, window, cx);
                        cx.notify();
                        cx.stop_propagation();
                    }
                }),
            )
    }

    fn workflow_tab(
        &self,
        kind: WorkflowKind,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(SharedString::from(format!("workflow-{}", kind.slug())))
            .role(Role::Tab)
            .aria_label(kind.label())
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .px_4()
            .py_2()
            .rounded_md()
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .bg(if selected {
                rgb(theme::ACCENT_DIM)
            } else {
                rgb(theme::SURFACE)
            })
            .border_1()
            .border_color(if selected {
                rgb(theme::ACCENT)
            } else {
                rgb(theme::BORDER)
            })
            .text_color(if selected {
                rgb(theme::TEXT)
            } else {
                rgb(theme::MUTED)
            })
            .hover(|s| {
                s.border_color(rgb(theme::ACCENT))
                    .text_color(rgb(theme::TEXT))
            })
            .child(kind.label())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.workflow = kind;
                this.start_key = None;
                cx.notify();
            }))
            // A tab must be reachable without a pointer, like every other control here.
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    this.workflow = kind;
                    this.start_key = None;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
    }

    /// Icon-rail navigation entry.
    fn nav_button(
        &self,
        id: &'static str,
        kind: theme::Icon,
        label: &'static str,
        page: Page,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let fg = if selected {
            theme::ACCENT
        } else {
            theme::MUTED
        };
        div()
            .id(id)
            .accessibility_id(id)
            .focusable()
            .tab_stop(true)
            .role(Role::Tab)
            .aria_label(format!("{label} page"))
            .aria_selected(selected)
            .cursor_pointer()
            .w(px(60.))
            .py_3()
            .rounded_lg()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .bg(if selected {
                rgb(theme::ACCENT_DIM)
            } else {
                rgb(theme::RAIL)
            })
            .border_1()
            .border_color(if selected {
                rgb(0x2c4d7d)
            } else {
                rgb(theme::RAIL)
            })
            .hover(|s| s.bg(rgb(theme::SURFACE_2)))
            .focus_visible(|s| s.border_color(rgb(theme::ACCENT)))
            .child(theme::icon(kind, fg))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(fg))
                    .font_weight(FontWeight::MEDIUM)
                    .child(label),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.state.navigate(page);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                    this.state.navigate(page);
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
    }

    /// A run's status chip, e.g. Completed / Running / Failed.
    fn outcome_pill(&self, snapshot: &RunSnapshot) -> Div {
        let name = format!("{:?}", snapshot.outcome);
        let (fg, bg) = theme::outcome_colors(&name);
        theme::pill(name, fg, bg)
    }

    fn mode_pill(&self) -> Div {
        match &self.mode {
            None => theme::pill("No mode · no inference", theme::MUTED, 0x1b3247),
            Some(Mode::Replay) => {
                theme::pill("Recorded replay · synthetic only", theme::ACCENT, 0x3a2418)
            }
            Some(Mode::Local { .. }) => {
                theme::pill("Local inference · explicit", theme::WARN, 0x33280f)
            }
            Some(Mode::Cloud { .. }) => {
                theme::pill("Cloud · paid per run", theme::VIOLET, 0x2a2340)
            }
        }
    }

    /// Page heading with an optional action area on the right.
    fn page_header(
        &self,
        title: &'static str,
        subtitle: impl Into<SharedString>,
        actions: Option<Div>,
    ) -> Div {
        let left = div()
            .flex()
            .flex_col()
            .gap_1()
            .flex_1()
            .child(
                div()
                    .id(SharedString::from(format!("page-title-{title}")))
                    .role(Role::Heading)
                    .aria_label(title)
                    .text_2xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT))
                    .child(title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(theme::MUTED))
                    .child(subtitle.into()),
            );
        if let Some(actions) = actions {
            return div()
                .flex()
                .flex_row()
                .items_start()
                .justify_between()
                .gap_6()
                .w_full()
                .child(left)
                .child(actions);
        }
        left
    }

    fn labelled(&self, label: &'static str, control: impl IntoElement) -> Div {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .w_full()
            .child(theme::micro(label))
            .child(control)
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div().flex().flex_col().gap_6().w_full();

        // Transient prompts are surfaced above the page, never as a modal trap.
        if let Some(id) = self.pending_open.clone() {
            content =
                content.child(
                    theme::panel()
                        .p_5()
                        .border_color(rgb(theme::WARN))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_sm().text_color(rgb(theme::TEXT)).child(
                            "Unsaved intake edits. Discard them and open the selected project?",
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .child(self.button(
                                    "discard-open",
                                    "Discard edits and open",
                                    move |this, _, cx| {
                                        this.pending_open = None;
                                        this.send(Request::Open(id.clone()), cx);
                                    },
                                    cx,
                                ))
                                .child(self.ghost_button(
                                    "keep-draft",
                                    "Keep editing",
                                    |this, _, cx| {
                                        this.pending_open = None;
                                        cx.notify();
                                    },
                                    cx,
                                )),
                        ),
                );
        }
        if self.confirm_close {
            content = content.child(
                theme::panel()
                    .p_5()
                    .border_color(rgb(theme::DANGER))
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(theme::TEXT))
                            .child("Quit now? Unsaved intake edits will be discarded. Every in-progress analysis this app owns, in any project, will be interrupted for explicit recovery/resume."),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(self.button(
                                "confirm-quit",
                                "Quit without saving",
                                |this, _, cx| {
                                    this.allow_close = true;
                                    cx.quit();
                                },
                                cx,
                            ))
                            .child(self.ghost_button(
                                "cancel-quit",
                                "Keep working",
                                |this, _, cx| {
                                    this.confirm_close = false;
                                    cx.notify();
                                },
                                cx,
                            )),
                    ),
            );
        }

        match self.state.page {
            Page::Projects => {
                content = content.child(self.page_header(
                    "Projects",
                    "Your standalone workspace. No legacy projects are imported automatically.",
                    Some(div().child(self.primary_button(
                        "new-project",
                        "New project",
                        |this, _, cx| {
                            if this.dirty(cx) {
                                this.state.notice = Some(
                                    "Save your current draft before creating another project."
                                        .into(),
                                );
                            } else {
                                this.state.selected_project = None;
                                this.state.run = None;
                                this.state.history.clear();
                                this.load_intake(&Intake::default(), cx);
                                this.state.page = Page::Intake;
                            }
                            cx.notify();
                        },
                        cx,
                    ))),
                ));
                if self.state.projects.is_empty() {
                    content = content.child(
                        theme::panel()
                            .p_10()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().text_lg().child("No projects yet"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(theme::MUTED))
                                    .child("Create one to begin. Startup never runs a model."),
                            ),
                    );
                } else {
                    let mut grid = div().flex().flex_row().flex_wrap().gap_4().w_full();
                    for project in self.state.projects.clone() {
                        let id = project.id.clone();
                        let words = project.intake.description.split_whitespace().count();
                        grid = grid.child(
                            div()
                                .id(SharedString::from(format!("project-{}", project.id)))
                                .role(Role::Button)
                                .aria_label(format!("Open project: {}", project.intake.title))
                                .focusable()
                                .tab_stop(true)
                                .cursor_pointer()
                                .w(px(330.))
                                .p_5()
                                .rounded_lg()
                                .bg(rgb(theme::SURFACE))
                                .border_1()
                                .border_color(rgb(theme::BORDER))
                                .shadow(theme::card_shadow())
                                .hover(|s| {
                                    s.border_color(rgb(theme::ACCENT)).bg(rgb(theme::SURFACE_2))
                                })
                                .focus_visible(|s| s.border_color(rgb(theme::ACCENT)))
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(
                                    div()
                                        .text_lg()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(rgb(theme::TEXT))
                                        .child(project.intake.title.clone()),
                                )
                                .child(theme::divider())
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .justify_between()
                                        .gap_2()
                                        .child(theme::micro(format!("{words} WORDS")))
                                        .child(theme::micro(theme::fmt_date(
                                            project.updated_at_ms,
                                        ))),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(theme::ACCENT))
                                        .child("Open project →"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.dirty(cx) {
                                        this.pending_open = Some(id.clone());
                                        cx.notify();
                                    } else {
                                        this.send(Request::Open(id.clone()), cx);
                                    }
                                })),
                        );
                    }
                    content = content.child(grid);
                }
            }
            Page::Intake => {
                let words = self
                    .description
                    .read(cx)
                    .buffer
                    .text()
                    .split_whitespace()
                    .count();
                content = content.child(self.page_header(
                    "Invention intake",
                    "Saved to this workspace only. Every field below is snapshotted into a run.",
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(self.button(
                                "save-intake",
                                "Save intake",
                                |this, _, cx| this.save(cx),
                                cx,
                            ))
                            .child(self.primary_button(
                                "start-intake",
                                "Start analysis",
                                |this, _, cx| this.start(cx),
                                cx,
                            )),
                    ),
                ));
                let mut columns = div().flex().flex_row().items_start().gap_4().w_full();
                let mut form = theme::panel().p_6().flex().flex_col().gap_4().flex_1();
                form = form
                    .child(self.labelled("TITLE", self.title.clone()))
                    .child(self.labelled("TECHNICAL FIELD", self.field.clone()))
                    .child(self.labelled("DESCRIPTION", self.description.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(theme::micro(format!("{words} WORDS")))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(if words >= 50 {
                                        theme::OK
                                    } else {
                                        theme::WARN
                                    }))
                                    .child(if words >= 50 {
                                        "meets the 50-word minimum"
                                    } else {
                                        "the start step requires at least 50 words"
                                    }),
                            ),
                    );
                columns = columns.child(form);

                let mut aside = div().w(px(300.)).flex().flex_col().gap_4().flex_shrink_0();
                aside =
                    aside
                        .child(
                            theme::panel()
                                .p_5()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(theme::micro("OPTIONAL CONTEXT"))
                                .child(div().text_sm().text_color(rgb(theme::MUTED)).child(
                                    "Nine legacy invention fields feed every analysis stage.",
                                ))
                                .child(self.button(
                                    "advanced-context",
                                    if self.advanced {
                                        "Hide optional fields"
                                    } else {
                                        "Edit optional fields"
                                    },
                                    |this, _, cx| {
                                        this.advanced = !this.advanced;
                                        cx.notify();
                                    },
                                    cx,
                                )),
                        )
                        .child(
                            theme::panel()
                                .p_5()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(theme::micro("WORKFLOW"))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(theme::TEXT))
                                        .child(self.workflow.label()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(theme::MUTED))
                                        .child(format!("{} stages", self.workflow.stage_count())),
                                ),
                        );
                columns = columns.child(aside);
                content = content.child(columns);

                if self.advanced {
                    let mut grid = div().flex().flex_row().flex_wrap().gap_4().w_full();
                    for (index, (label, _)) in InventionContext::default()
                        .labeled_fields()
                        .into_iter()
                        .enumerate()
                    {
                        grid = grid.child(
                            div()
                                .w(px(300.))
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(theme::micro(label.to_uppercase()))
                                .child(self.context[index].clone()),
                        );
                    }
                    content = content.child(
                        theme::panel()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .w_full()
                            .child(theme::micro("OPTIONAL INVENTION CONTEXT"))
                            .child(grid),
                    );
                }
            }
            Page::Workflow => {
                content = content.child(self.page_header(
                    self.workflow.label(),
                    if self.workflow == WorkflowKind::Compliance {
                        format!(
                            "{} checks. Reviews the claim set against the feasibility analysis — it does not review a drafted specification yet.",
                            self.workflow.stage_count()
                        )
                    } else {
                        format!(
                            "{} stages. A workflow that consumes another needs that analysis complete first.",
                            self.workflow.stage_count()
                        )
                    },
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(self.primary_button(
                                "new-run",
                                "Start new version",
                                |this, _, cx| this.start(cx),
                                cx,
                            )),
                    ),
                ));

                let mut tabs = div()
                    .id("workflow-tabs")
                    .accessibility_id("workflow-tabs")
                    .role(Role::TabList)
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_2()
                    .w_full();
                for kind in WorkflowKind::ALL {
                    tabs = tabs.child(self.workflow_tab(kind, kind == self.workflow, cx));
                }
                content = content.child(tabs);

                // Compliance can be started against chosen revisions; the selection is
                // recorded in the run's immutable input.
                if self.workflow == WorkflowKind::Compliance {
                    let claims = self.compliance_claims.clone();
                    let specification = self.compliance_specification.clone();
                    let claims_label = if claims.is_some() {
                        "the selected claims revision"
                    } else {
                        "the latest claims run"
                    };
                    let spec_label = if specification.is_some() {
                        "the selected specification revision"
                    } else {
                        "the feasibility analysis"
                    };
                    content = content.child(
                        theme::panel()
                            .id("compliance-selection")
                            .accessibility_id("compliance-selection")
                            .p_5()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(theme::micro("REVISIONS UNDER REVIEW"))
                            .child(div().text_xs().text_color(rgb(theme::MUTED)).child(format!(
                                "Claims: {claims_label} · Specification: {spec_label}"
                            )))
                            .child(self.primary_button(
                                "start-selected-compliance",
                                "Start compliance with selected revisions",
                                move |this, _, cx| {
                                    let Some(project) =
                                        this.state.selected_project.as_ref().map(|p| p.id.clone())
                                    else {
                                        this.state.notice =
                                            Some("Open a project before starting a run.".into());
                                        return;
                                    };
                                    if !this.synthetic && !this.real_data {
                                        this.state.notice = Some(
                                            "Declare the data in Execution setup before starting."
                                                .into(),
                                        );
                                        return;
                                    }
                                    let key = this
                                        .start_key
                                        .get_or_insert_with(|| RunId::new().to_string())
                                        .clone();
                                    this.pending = true;
                                    this.send(
                                        Request::StartSelected {
                                            project,
                                            kind: WorkflowKind::Compliance,
                                            idempotency_key: key,
                                            authorization: Authorization {
                                                synthetic_data: this.synthetic,
                                                real_data: this.real_data,
                                                local_inference: this.authorized_local,
                                                paid_provider: this.cloud_approved,
                                                research: this.research_authorized,
                                            },
                                            claims_revision: claims.clone(),
                                            specification_revision: specification.clone(),
                                        },
                                        cx,
                                    );
                                },
                                cx,
                            )),
                    );
                }

                // Editing surface: only the two document workflows can be edited. A run
                // is immutable; an edit becomes the next numbered revision anchored to
                // the latest completed run of that kind.
                let editable = match self.workflow {
                    WorkflowKind::Claims => Some(DocumentKind::Claims),
                    WorkflowKind::Application => Some(DocumentKind::Application),
                    _ => None,
                };
                if let (Some(kind), true) = (editable, self.state.selected_project.is_some()) {
                    {
                        let mut panel =
                            theme::panel()
                                .id("revision-panel")
                                .accessibility_id("revision-panel")
                                .p_5()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .child(theme::micro("EDITIONS OF THIS DOCUMENT"))
                                .child(self.labelled("REVISION TEXT", self.revision_text.clone()))
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap_2()
                                        .child(self.button(
                                            "load-run-text",
                                            "Load run text",
                                            |this, _, cx| {
                                                let text = this
                                                    .state
                                                    .run
                                                    .as_ref()
                                                    .and_then(|view| {
                                                        view.snapshot.checkpoints.last().map(
                                                            |checkpoint| checkpoint.text.clone(),
                                                        )
                                                    })
                                                    .unwrap_or_default();
                                                this.revision_text
                                                    .update(cx, |input, cx| input.load(text, cx));
                                                this.state.notice = Some(
                                                    "Loaded the run's final stage into the editor."
                                                        .into(),
                                                );
                                                cx.notify();
                                            },
                                            cx,
                                        ))
                                        .child(self.primary_button(
                                            "save-revision",
                                            "Save revision",
                                            move |this, _, cx| {
                                                let text = this
                                                    .revision_text
                                                    .read(cx)
                                                    .buffer
                                                    .text()
                                                    .to_string();
                                                let project = this
                                                    .state
                                                    .selected_project
                                                    .as_ref()
                                                    .map(|project| project.id.clone());
                                                match project {
                                                    Some(project) => this.send(
                                                        Request::SaveRevision {
                                                            project,
                                                            kind,
                                                            text,
                                                            note: "edited in the app".into(),
                                                        },
                                                        cx,
                                                    ),
                                                    None => this.state.notice = Some(
                                                        "Open a project before saving a revision."
                                                            .into(),
                                                    ),
                                                }
                                            },
                                            cx,
                                        ))
                                        .child(self.button(
                                            "list-revisions",
                                            "List revisions",
                                            move |this, _, cx| {
                                                let project = this
                                                    .state
                                                    .selected_project
                                                    .as_ref()
                                                    .map(|project| project.id.clone());
                                                if let Some(project) = project {
                                                    this.send(
                                                        Request::ListRevisions { project, kind },
                                                        cx,
                                                    );
                                                }
                                            },
                                            cx,
                                        )),
                                );
                        // Rows carry a compare action that diffs the previous version of the
                        // same kind against this one. Built from cloned data so the closures
                        // can mutate the view.
                        let mut rows: Vec<(u32, String, String, Option<String>, String)> =
                            Vec::new();
                        {
                            let series: Vec<DocumentRevision> = self
                                .revisions
                                .iter()
                                .filter(|revision| revision.kind == kind)
                                .cloned()
                                .collect();
                            for (index, entry) in series.iter().enumerate() {
                                let base = entry
                                    .base_run_id
                                    .as_ref()
                                    .map(|run| run.0.chars().take(8).collect::<String>())
                                    .unwrap_or_else(|| "manual".into());
                                let previous = index
                                    .checked_sub(1)
                                    .and_then(|prior| series.get(prior))
                                    .map(|prior| prior.text.clone());
                                rows.push((
                                    entry.version,
                                    base,
                                    entry.note.clone(),
                                    previous,
                                    entry.text.clone(),
                                ));
                            }
                        }
                        for (version, base, note, previous, text) in rows {
                            let mut row = div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_x_2()
                                .gap_y_2()
                                .child(theme::pill(format!("v{version}"), theme::ACCENT, 0x3a2418))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(theme::MUTED))
                                        .child(format!("base {base} · {note}")),
                                );
                            let revision_id = self
                                .revisions
                                .iter()
                                .find(|entry| entry.kind == kind && entry.version == version)
                                .map(|entry| entry.id.clone());
                            if let Some(revision_id) = revision_id {
                                let label = format!("Use v{version} for compliance");
                                row = row.child(self.button(
                                    format!("use-v{version}-for-compliance"),
                                    label,
                                    move |this, _, cx| {
                                        match kind {
                                            DocumentKind::Claims => {
                                                this.compliance_claims = Some(revision_id.clone())
                                            }
                                            DocumentKind::Application => {
                                                this.compliance_specification =
                                                    Some(revision_id.clone())
                                            }
                                        }
                                        this.state.notice = Some(format!(
                                            "Compliance will review {} revision {version}.",
                                            kind.slug()
                                        ));
                                        cx.notify();
                                    },
                                    cx,
                                ));
                            }
                            if let Some(previous) = previous {
                                // Bounded by the diff itself: the prefix and suffix are
                                // trimmed, and an enormous change is reported coarsely rather
                                // than allocating a table over the whole document.
                                let comparison = pfl_workflow::diff::diff_text(&previous, &text);
                                let added = comparison.added;
                                let removed = comparison.removed;
                                let coarse = comparison.oversized;
                                row = row.child(self.button(
                                    format!("compare-v{version}"),
                                    format!("Compare v{version} with v{}", version - 1),
                                    move |this, _, cx| {
                                        this.revision_diff = Some(comparison.clone());
                                        this.state.notice = Some(format!(
                                            "Diff v{} to v{}: {added} added, {removed} removed{}",
                                            version - 1,
                                            version,
                                            if coarse {
                                                ", approximate: these revisions are too large for an exact comparison"
                                            } else {
                                                ""
                                            }
                                        ));
                                        cx.notify();
                                    },
                                    cx,
                                ));
                            }
                            panel = panel.child(row);
                        }
                        if let Some(diff) = &self.revision_diff {
                            let mut block = div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .p_3()
                                .bg(rgb(0x11202e))
                                .rounded_md();
                            for line in diff.lines.iter().take(200) {
                                let (prefix, colour) = match line.kind {
                                    pfl_workflow::diff::DiffKind::Added => ("+", theme::ACCENT),
                                    pfl_workflow::diff::DiffKind::Removed => ("-", theme::DANGER),
                                    pfl_workflow::diff::DiffKind::Same => (" ", theme::MUTED),
                                };
                                block = block.child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(colour))
                                        .child(format!("{prefix} {}", line.text)),
                                );
                            }
                            panel = panel.child(block);
                        }
                        let claims_selection = self.compliance_claims.clone();
                        let specification_selection = self.compliance_specification.clone();
                        panel = panel.child(self.button(
                            "export-package",
                            "Export revisions package",
                            move |this, _, cx| {
                                let Some(project) =
                                    this.state.selected_project.as_ref().map(|p| p.id.clone())
                                else {
                                    this.state.notice =
                                        Some("Open a project before exporting.".into());
                                    return;
                                };
                                let stamp = pfl_domain::now_ms();
                                let dir = data_directory()
                                    .map(|root| root.join("exports").join(stamp.to_string()))
                                    .unwrap_or_else(|_| std::env::temp_dir().join("pfl-exports"));
                                this.send(
                                    Request::ExportPackage {
                                        project,
                                        claims: claims_selection.clone(),
                                        application: specification_selection.clone(),
                                        dir,
                                    },
                                    cx,
                                );
                            },
                            cx,
                        ));
                        content = content.child(panel);
                    }
                }

                // Before a paid run, the window states the model, that the total is unknown
                // until usage arrives, and the rates that will be applied.
                if self.cloud_approved {
                    let model = self.cloud_model.read(cx).buffer.text().to_string();
                    let pricing = pfl_workflow::pricing_for(&model);
                    let text = pfl_workflow::confirmation_text(&model, pricing.as_ref());
                    content = content.child(
                        div()
                            .id("cost-confirmation")
                            .accessibility_id("cost-confirmation")
                            .role(Role::Status)
                            .aria_label(text.clone())
                            .p_4()
                            .rounded_md()
                            .bg(rgb(0x1b3247))
                            .text_xs()
                            .text_color(rgb(theme::MUTED))
                            .child(text),
                    );
                }

                // Suffix regeneration: offer a restart point at each committed stage. The
                // copy never says "single stage"; everything from the choice onward re-runs.
                if let Some(view) = self
                    .state
                    .run
                    .as_ref()
                    .filter(|view| view.snapshot.outcome.is_terminal())
                {
                    {
                        let run_id = view.snapshot.id.clone();
                        let mut row = div().flex().flex_row().flex_wrap().gap_2();
                        for checkpoint in &view.snapshot.checkpoints {
                            let stage = checkpoint.stage;
                            let run = run_id.clone();
                            row = row.child(self.button(
                                format!("regen-{}", stage.number()),
                                format!("Regenerate from {} onward", stage.label()),
                                move |this, _, cx| {
                                    // Regenerating spends again, so it passes the same gate as
                                    // starting: without a fresh approval the click does nothing
                                    // but explain itself.
                                    if this.gate_blocks(cx) {
                                        return;
                                    }
                                    this.send(
                                        Request::RegenerateFrom {
                                            run: run.clone(),
                                            stage,
                                            authorization: this.authorization(),
                                        },
                                        cx,
                                    );
                                },
                                cx,
                            ));
                        }
                        content = content.child(row);
                    }
                }

                // References recorded on this run, straight from its immutable input.
                if let Some(view) = self
                    .state
                    .run
                    .as_ref()
                    .filter(|view| !view.snapshot.input.upstream.prior_art.is_empty())
                {
                    {
                        let mut list = theme::panel()
                            .id("prior-art")
                            .accessibility_id("prior-art")
                            .p_5()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(theme::micro("REFERENCES RECORDED FOR THIS RUN"));
                        for (index, item) in
                            view.snapshot.input.upstream.prior_art.iter().enumerate()
                        {
                            let dates = if item.grant_date.trim().is_empty() {
                                String::new()
                            } else {
                                format!(" · {}", item.grant_date)
                            };
                            let source = if item.source.trim().is_empty() {
                                String::new()
                            } else {
                                format!(" · {}", item.source)
                            };
                            let status = if item.verified {
                                "verified"
                            } else {
                                "unverified"
                            };
                            list = list.child(
                                div()
                                    .id(format!("prior-art-{index}"))
                                    .accessibility_id(format!("prior-art-entry-{index}"))
                                    .role(Role::Group)
                                    .aria_label(format!(
                                        "Reference {}: {}",
                                        item.reference, item.title
                                    ))
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(theme::TEXT))
                                            .child(format!("{} — {}", item.reference, item.title)),
                                    )
                                    .child(div().text_xs().text_color(rgb(theme::MUTED)).child(
                                        format!("{}{}{} · {status}", item.assignee, dates, source),
                                    )),
                            );
                        }
                        content = content.child(list);
                    }
                }

                if let Some(view) = &self.state.run {
                    let total = view.snapshot.input.kind.stage_count();
                    let done = view.snapshot.checkpoints.len();
                    let (fg, _) = theme::outcome_colors(&format!("{:?}", view.snapshot.outcome));

                    let mut summary = theme::panel()
                        .id("run-summary")
                        .accessibility_id("run-summary")
                        .role(Role::Status)
                        .aria_label(format!(
                            "Run status: {:?}, {} of {} checkpoints, version {}, {} stages left",
                            view.snapshot.outcome,
                            done,
                            total,
                            view.snapshot.version,
                            total.saturating_sub(done)
                        ))
                        .p_6()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .w_full()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_end()
                                        .gap_3()
                                        .child(
                                            div()
                                                .text_3xl()
                                                .font_weight(FontWeight::BOLD)
                                                .text_color(rgb(fg))
                                                .child(format!("{done}/{total}")),
                                        )
                                        .child(
                                            div()
                                                .pb_1()
                                                .text_sm()
                                                .text_color(rgb(theme::MUTED))
                                                .child("checkpoints"),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap_2()
                                        .child(self.outcome_pill(&view.snapshot))
                                        .child(theme::pill(
                                            format!(
                                                "v{} · {}",
                                                view.snapshot.version,
                                                view.snapshot.input.kind.slug()
                                            ),
                                            theme::MUTED,
                                            0x1b3247,
                                        )),
                                ),
                        )
                        .child(theme::progress(done, total, fg))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(theme::micro("PROVIDER"))
                                        .child(div().text_sm().text_color(rgb(theme::TEXT)).child(
                                            format!(
                                                "{:?} · {}",
                                                view.snapshot.input.provider.kind,
                                                view.snapshot.input.provider.model
                                            ),
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(theme::micro("EVENT"))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(theme::TEXT))
                                                .child(view.sequence.to_string()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(theme::micro("ATTEMPTS"))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(theme::TEXT))
                                                .child(view.snapshot.attempts.len().to_string()),
                                        ),
                                ),
                        );

                    if let Some(message) = &view.message {
                        summary = summary.child(
                            div()
                                .text_sm()
                                .text_color(rgb(theme::WARN))
                                .child(message.clone()),
                        );
                    }
                    let id = view.snapshot.id.clone();
                    let mut actions = div().flex().flex_row().gap_2();
                    if view.can_cancel() {
                        actions = actions.child(self.button(
                            "cancel-run",
                            "Cancel run",
                            move |this, _, cx| {
                                this.send(Request::Cancel(id.clone()), cx);
                            },
                            cx,
                        ));
                    } else if view.can_resume() {
                        actions = actions.child(self.button(
                            "resume-run",
                            "Resume this run",
                            move |this, _, cx| {
                                if (!this.synthetic && !this.real_data)
                                    || (matches!(this.mode, Some(Mode::Local { .. }))
                                        && !this.authorized_local)
                                {
                                    this.state.notice = Some("Declare the data and authorize local inference in Execution setup before resuming.".into());
                                    cx.notify();
                                    return;
                                }
                                if this.gate_blocks(cx) {
                                    return;
                                }
                                this.send(
                                    Request::Resume {
                                        run: id.clone(),
                                        authorization: this.authorization(),
                                    },
                                    cx,
                                );
                            },
                            cx,
                        ));
                    }
                    summary = summary.child(actions);
                    content = content.child(summary);

                    // Stage ledger: one row per stage, done / current / pending.
                    let mut stages = theme::panel().p_2().flex().flex_col().w_full();
                    for (index, stage) in view.snapshot.input.kind.stages().iter().enumerate() {
                        let checkpoint = view
                            .snapshot
                            .checkpoints
                            .iter()
                            .find(|checkpoint| checkpoint.stage == *stage);
                        let is_current = checkpoint.is_none() && index == done;
                        let (label_color, state_text) = match checkpoint {
                            Some(_) => (theme::TEXT, "done"),
                            None if is_current => (theme::ACCENT, "running"),
                            None => (theme::FAINT, "pending"),
                        };
                        let chars = checkpoint.map(|c| c.text.chars().count());
                        stages = stages.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .px_4()
                                .py_3()
                                .rounded_md()
                                .when(is_current, |s| s.bg(rgb(theme::SURFACE_2)))
                                .child(
                                    div()
                                        .w(px(22.))
                                        .text_xs()
                                        .text_color(rgb(theme::FAINT))
                                        .child(format!("{:02}", index + 1)),
                                )
                                .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(rgb(
                                    match checkpoint {
                                        Some(_) => theme::OK,
                                        None if is_current => theme::ACCENT,
                                        None => theme::BORDER,
                                    },
                                )))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_sm()
                                        .text_color(rgb(label_color))
                                        .child(stage.label()),
                                )
                                .child(div().text_xs().text_color(rgb(theme::FAINT)).child(
                                    match chars {
                                        Some(chars) => format!("{chars} chars"),
                                        None => state_text.to_string(),
                                    },
                                )),
                        );
                    }
                    content = content.child(stages);

                    content = content.child(
                        theme::panel()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .w_full()
                            .child(theme::micro("STAGE OUTPUT"))
                            .child(self.stream.clone()),
                    );
                } else {
                    content = content.child(
                        theme::panel()
                            .p_10()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().text_lg().child("No run selected"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(theme::MUTED))
                                    .child("Save intake, configure execution and explicitly start an analysis."),
                            ),
                    );
                }
            }
            Page::Report => {
                let meta = self.state.run.as_ref().map(|view| {
                    (
                        view.snapshot.version,
                        view.snapshot.id.to_string(),
                        view.snapshot
                            .report
                            .as_ref()
                            .map(|report| report.markdown.chars().count()),
                    )
                });
                content = content.child(self.page_header(
                    "Report",
                    "Research assistance, not legal advice. Review with a qualified patent professional.",
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(self.button(
                                "export-md",
                                "Markdown",
                                |this, _, cx| this.export(ExportFormat::Markdown, cx),
                                cx,
                            ))
                            .child(self.button(
                                "export-html",
                                "HTML",
                                |this, _, cx| this.export(ExportFormat::Html, cx),
                                cx,
                            ))
                            .child(self.primary_button(
                                "export-word",
                                "Export Word",
                                |this, _, cx| this.export(ExportFormat::Word, cx),
                                cx,
                            ))
                            .child(self.button(
                                "toggle-report-view",
                                if self.report_rendered {
                                    "View source"
                                } else {
                                    "Rendered view"
                                },
                                |this, _, cx| {
                                    this.report_rendered = !this.report_rendered;
                                    cx.notify();
                                },
                                cx,
                            )),
                    ),
                ));

                if let Some((version, id, chars)) = meta {
                    content = content.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(theme::pill(
                                format!("version {version}"),
                                theme::ACCENT,
                                0x3a2418,
                            ))
                            .child(theme::pill(
                                id.chars().take(18).collect::<String>(),
                                theme::MUTED,
                                0x1b3247,
                            ))
                            .child(match chars {
                                Some(chars) => {
                                    theme::pill(format!("{chars} chars"), theme::MUTED, 0x1b3247)
                                }
                                None => theme::pill("no saved report", theme::WARN, 0x33280f),
                            }),
                    );
                    if self.report_rendered {
                        let markdown = self
                            .state
                            .run
                            .as_ref()
                            .and_then(|view| view.snapshot.report.as_ref())
                            .map(|report| report.markdown.clone())
                            .unwrap_or_default();
                        content = content.child(
                            theme::panel()
                                .id("report-document")
                                .accessibility_id("report-document")
                                .role(Role::Document)
                                .aria_label("Rendered saved report")
                                .bg(rgb(theme::DOC_BG))
                                .border_color(rgb(theme::DOC_RULE))
                                .text_color(rgb(theme::DOC_TEXT))
                                .p_8()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .w_full()
                                .child(report_document(&markdown)),
                        );
                    } else {
                        content = content.child(
                            theme::panel()
                                .p_8()
                                .flex()
                                .flex_col()
                                .gap_4()
                                .w_full()
                                .child(self.report.clone()),
                        );
                    }
                } else {
                    content = content.child(
                        theme::panel()
                            .p_10()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().text_lg().child("No saved report selected"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(theme::MUTED))
                                    .child("Open a project or choose a version in History."),
                            ),
                    );
                }
            }
            Page::History => {
                content = content.child(self.page_header(
                    "History",
                    format!(
                        "{} saved version{} in this project. Reading history never starts work.",
                        self.state.history.len(),
                        if self.state.history.len() == 1 {
                            ""
                        } else {
                            "s"
                        }
                    ),
                    None,
                ));
                if self.state.history.is_empty() {
                    content = content.child(
                        theme::panel()
                            .p_10()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .w_full()
                            .child(div().text_lg().child("No saved runs yet"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(theme::MUTED))
                                    .child("Reopen the project to refresh its history."),
                            ),
                    );
                } else {
                    let mut list = div().flex().flex_col().gap_2().w_full();
                    for snapshot in self.state.history.clone() {
                        let id = snapshot.id.clone();
                        let (fg, _) = theme::outcome_colors(&format!("{:?}", snapshot.outcome));
                        list = list.child(
                            div()
                                .id(SharedString::from(format!("history-{id}")))
                                .role(Role::Button)
                                .aria_label(format!(
                                    "Open version {} ({})",
                                    snapshot.version,
                                    snapshot.input.kind.label()
                                ))
                                .focusable()
                                .tab_stop(true)
                                .cursor_pointer()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_4()
                                .px_5()
                                .py_4()
                                .rounded_lg()
                                .bg(rgb(theme::SURFACE))
                                .border_1()
                                .border_color(rgb(theme::BORDER))
                                .hover(|s| {
                                    s.border_color(rgb(theme::ACCENT)).bg(rgb(theme::SURFACE_2))
                                })
                                .focus_visible(|s| s.border_color(rgb(theme::ACCENT)))
                                .child(
                                    div()
                                        .w(px(44.))
                                        .text_lg()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(theme::TEXT))
                                        .child(format!("v{}", snapshot.version)),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .flex_1()
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(rgb(theme::TEXT))
                                                .child(snapshot.input.kind.label()),
                                        )
                                        .child(
                                            div().text_xs().text_color(rgb(theme::MUTED)).child(
                                                format!(
                                                    "{}/{} checkpoints · {:?} · {}",
                                                    snapshot.checkpoints.len(),
                                                    snapshot.input.kind.stage_count(),
                                                    snapshot.input.provider.kind,
                                                    theme::fmt_date(snapshot.created_at_ms)
                                                ),
                                            ),
                                        ),
                                )
                                .child(theme::pill(format!("{:?}", snapshot.outcome), fg, 0x1a2431))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(theme::ACCENT))
                                        .child("Open →"),
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.state.page = Page::Report;
                                    this.send(Request::SelectRun(id.clone()), cx);
                                })),
                        );
                    }
                    content = content.child(list);
                }
            }
            Page::Models => {
                content = content.child(self.page_header(
                    "Models",
                    "What can run each stage: the registry, what is answering on this machine, and whether the CLIs are signed in.",
                    Some(div().flex().gap_2().child(self.primary_button(
                        "refresh-models",
                        "Refresh",
                        |this, _, cx| {
                            this.send(Request::DiscoverProviders, cx);
                            this.send(Request::ListConnections, cx);
                            if let Some(project) =
                                this.state.selected_project.as_ref().map(|p| p.id.clone())
                            {
                                this.send(Request::ListAssignments { project }, cx);
                            }
                        },
                        cx,
                    ))
                    .child(if self.state.selected_project.is_some() {
                        self.button(
                            "models-automatic",
                            "Automatic for every workflow",
                            |this, _, cx| {
                                if let Some(project) =
                                    this.state.selected_project.as_ref().map(|p| p.id.clone())
                                {
                                    for kind in pfl_domain::WorkflowKind::ALL {
                                        this.send(
                                            Request::ClearAssignments {
                                                project: project.clone(),
                                                surface: format!("workflow-{}", kind.slug()),
                                            },
                                            cx,
                                        );
                                    }
                                }
                            },
                            cx,
                        )
                        .into_any_element()
                    } else {
                        div().into_any_element()
                    })),
                ));
                let mut list = div().flex().flex_col().gap_2().w_full();
                // Adding a connection records an endpoint; nothing is contacted until a run uses it.
                list = list.child(
                    theme::panel()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(self.labelled("CONNECTION NAME", self.connection_name.clone()))
                        .child(self.labelled("BASE URL", self.connection_url.clone()))
                        .child(self.button(
                            "add-connection",
                            "Save connection",
                            |this, _, cx| {
                                let name = this.connection_name.read(cx).buffer.text().to_string();
                                let base_url =
                                    this.connection_url.read(cx).buffer.text().to_string();
                                this.send(
                                    Request::AddConnection {
                                        name,
                                        base_url,
                                        model: None,
                                    },
                                    cx,
                                );
                            },
                            cx,
                        )),
                );
                // Saved endpoints, addressed by a run as `conn-<id>`.
                if !self.connections.is_empty() {
                    let mut saved = div().flex().flex_col().gap_1().w_full();
                    for connection in &self.connections {
                        saved = saved.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .text_xs()
                                .text_color(rgb(theme::MUTED))
                                .child(connection.name.clone())
                                .child(connection.id.clone())
                                .child(
                                    div()
                                        .text_color(rgb(theme::TEXT))
                                        .child(connection.base_url.clone()),
                                ),
                        );
                    }
                    list = list.child(theme::panel().p_4().child(saved));
                }
                // What the project currently says, in one place: the page marked rows "In use"
                // but never listed the record itself.
                if !self.assignments.is_empty() {
                    let mut chosen = div().flex().flex_col().gap_1().w_full();
                    for row in &self.assignments {
                        chosen = chosen.child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_2()
                                .text_xs()
                                .text_color(rgb(theme::MUTED))
                                .child(format!("{} · rank {}", row.surface, row.rank))
                                .child(
                                    div()
                                        .text_color(rgb(theme::TEXT))
                                        .child(row.provider.clone()),
                                ),
                        );
                    }
                    list = list.child(theme::panel().p_4().child(chosen));
                }
                if self.providers.is_empty() {
                    list = list.child(
                        theme::panel().p_4().child(
                            div()
                                .text_sm()
                                .text_color(rgb(theme::MUTED))
                                .child("Nothing scanned yet. Refresh to look for local servers and signed-in programs."),
                        ),
                    );
                }
                for provider in &self.providers {
                    let status = if provider.available {
                        "available".to_string()
                    } else {
                        provider
                            .note
                            .clone()
                            .unwrap_or_else(|| "unavailable".to_string())
                    };
                    list = list.child(
                        theme::panel().p_4().child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(rgb(theme::TEXT))
                                        .child(provider.label.clone()),
                                )
                                .child(div().text_xs().text_color(rgb(theme::MUTED)).child(status))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(theme::FAINT))
                                        .child(format!("{} model(s)", provider.models.len())),
                                )
                                .child(self.provider_choice_button(provider, cx))
                                .child(self.provider_fallback_button(provider, cx))
                                .child(self.provider_override_button(provider, cx))
                                .child(self.provider_stage_button(provider, cx)),
                        ),
                    );
                }
                content = content.child(list);
            }
            Page::Settings => {
                content = content.child(self.page_header(
                    "Execution setup",
                    "Choose explicitly. Configuration does not send a request, start Ollama or pull a model.",
                    None,
                ));
                let mut columns = div().flex().flex_row().items_start().gap_4().w_full();

                let mut mode_card = theme::panel()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .flex_1()
                    .child(theme::micro("EXECUTION MODE"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(self.mode_pill()),
                    )
                    .child(self.button(
                        "replay-mode",
                        "Use synthetic recorded replay",
                        |this, _, cx| {
                            this.send(Request::Configure(Mode::Replay), cx);
                        },
                        cx,
                    ))
                    .child(theme::divider())
                    .child(self.labelled(
                        "LOCAL OLLAMA ENDPOINT (ON-MACHINE ONLY)",
                        self.endpoint.clone(),
                    ))
                    .child(self.labelled("ALREADY-INSTALLED MODEL NAME", self.model.clone()))
                    .child(self.button(
                        "list-models",
                        "List installed models",
                        |this, _, cx| {
                            let endpoint = this.endpoint.read(cx).buffer.text().to_string();
                            this.local_status = "Querying the runtime...".into();
                            this.send(Request::ListLocalModels { endpoint }, cx);
                        },
                        cx,
                    ))
                    .child(
                        div()
                            .id("local-readiness")
                            .accessibility_id("local-readiness")
                            .role(Role::Status)
                            .aria_label(self.local_status.clone())
                            .text_xs()
                            .text_color(rgb(theme::MUTED))
                            .child(self.local_status.clone()),
                    )
                    .child({
                        let mut row = div().flex().flex_row().flex_wrap().gap_2();
                        for (index, name) in self.local_models.clone().into_iter().enumerate() {
                            let picked = name.clone();
                            row = row.child(self.button(
                                format!("pick-model-{index}"),
                                name.clone(),
                                move |this, _, cx| {
                                    this.model
                                        .update(cx, |input, cx| input.load(picked.clone(), cx));
                                    this.state.notice = Some(format!("Model {picked} selected."));
                                    cx.notify();
                                },
                                cx,
                            ));
                        }
                        row
                    })
                    .child(self.primary_button(
                        "local-mode",
                        "Configure existing local runtime",
                        |this, _, cx| {
                            if this.profile == SetupProfile::Lean {
                                this.state.notice = Some(
                                    "The Lean profile refuses a local runtime. Choose Full in Setup to use your own.".into(),
                                );
                                cx.notify();
                                return;
                            }
                            let mode = Mode::Local {
                                endpoint: this.endpoint.read(cx).buffer.text().into(),
                                model: this.model.read(cx).buffer.text().into(),
                            };
                            this.send(Request::Configure(mode), cx);
                        },
                        cx,
                    ));
                mode_card = mode_card.child(
                    div()
                        .text_xs()
                        .text_color(rgb(theme::FAINT))
                        .child("A remote endpoint is not on-machine inference. Nothing is installed or downloaded."),
                );
                mode_card = mode_card
                    .child(theme::divider())
                    .child(theme::micro("CLOUD (ANTHROPIC) · PAID PER RUN"))
                    .child(self.labelled("CLOUD MODEL NAME", self.cloud_model.clone()))
                    .child(self.labelled("CLOUD API KEY (SESSION ONLY)", self.cloud_key.clone()))
                    .child(div().text_xs().text_color(rgb(theme::FAINT)).child({
                        // The confirmation line is computed from the model the
                        // operator typed, so it names the model and either the
                        // rates that will apply or says plainly that none are
                        // configured. The total stays unknown until usage arrives.
                        let model = self.cloud_model.read(cx).buffer.text().to_string();
                        let pricing = pfl_workflow::pricing_for(&model);
                        let text = pfl_workflow::confirmation_text(&model, pricing.as_ref());
                        div()
                            .id("cost-confirmation")
                            .accessibility_id("cost-confirmation")
                            .role(Role::Status)
                            .aria_label(text.clone())
                            .child(text)
                    }))
                    .child(self.labelled(
                        "PATENTSVIEW API KEY (SESSION ONLY)",
                        self.research_key.clone(),
                    ))
                    .child(self.button(
                        "research-config",
                        "Configure PatentsView research",
                        |this, _, cx| {
                            let key = this.research_key.read(cx).buffer.text().to_string();
                            match this.session.request(Request::ConfigureResearch {
                                api_key: crate::session::Secret::new(key),
                            }) {
                                Ok(()) => {
                                    this.state.notice = Some(
                                        "Research configured. It runs only for a run that opts in."
                                            .into(),
                                    )
                                }
                                Err(error) => this.state.notice = Some(error.to_string()),
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(self.primary_button(
                        "cloud-mode",
                        "Configure Cloud with this key",
                        |this, _, cx| {
                            let model = this.cloud_model.read(cx).buffer.text().to_string();
                            let key = this.cloud_key.read(cx).buffer.text().to_string();
                            // The key travels to the session actor and nowhere else.
                            this.cloud_approved = false;
                            this.send(
                                Request::ConfigureCloud {
                                    model,
                                    key: crate::session::Secret::new(key),
                                },
                                cx,
                            );
                        },
                        cx,
                    ));

                let consents = theme::panel()
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .w(px(330.))
                    .flex_shrink_0()
                    .child(theme::micro("AUTHORIZATION"))
                    .child(self.button(
                        "real-data-consent",
                        if self.real_data {
                            "✓  Real invention data (stays on this machine)"
                        } else {
                            "○  Real invention data (stays on this machine)"
                        },
                        |this, _, cx| {
                            this.real_data = !this.real_data;
                            if this.real_data {
                                this.synthetic = false;
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(self.button(
                        "synthetic-consent",
                        if self.synthetic {
                            "✓  Synthetic test data"
                        } else {
                            "○  Synthetic test data"
                        },
                        |this, _, cx| {
                            this.synthetic = !this.synthetic;
                            if this.synthetic {
                                this.real_data = false;
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(self.button(
                        "local-consent",
                        if self.authorized_local {
                            "✓  Authorize local inference"
                        } else {
                            "○  Authorize local inference"
                        },
                        |this, _, cx| {
                            this.authorized_local = !this.authorized_local;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(self.button(
                        "research-consent",
                        if self.research_authorized {
                            "✓  Use PatentsView research for this run"
                        } else {
                            "○  Use PatentsView research for this run"
                        },
                        |this, _, cx| {
                            this.research_authorized = !this.research_authorized;
                            // Opting in also installs the key, so the run cannot be
                            // authorized for research that was never configured.
                            if this.research_authorized {
                                let key = this.research_key.read(cx).buffer.text().to_string();
                                if !key.trim().is_empty() && !this.research_configured {
                                    this.send(
                                        Request::ConfigureResearch {
                                            api_key: crate::session::Secret::new(key),
                                        },
                                        cx,
                                    );
                                    this.research_configured = true;
                                }
                            }
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(self.button(
                        "cloud-consent",
                        if self.cloud_approved {
                            "✓  Approve paid Cloud request (this run)"
                        } else {
                            "○  Approve paid Cloud request (this run)"
                        },
                        |this, _, cx| {
                            this.cloud_approved = !this.cloud_approved;
                            cx.notify();
                        },
                        cx,
                    ))
                    .child(theme::divider())
                    .child(theme::micro("SETUP PROFILE"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(self.button(
                                "profile-lean",
                                if self.profile == SetupProfile::Lean {
                                    "• Lean — Cloud setup, no local runtime"
                                } else {
                                    "Lean — Cloud setup, no local runtime"
                                },
                                |this, _, cx| {
                                    this.send(Request::SetProfile(SetupProfile::Lean), cx);
                                    this.state.notice = Some("Setup profile set to Lean.".into());
                                },
                                cx,
                            ))
                            .child(self.button(
                                "profile-full",
                                if self.profile == SetupProfile::Full {
                                    "• Full — Local and Cloud"
                                } else {
                                    "Full — Local and Cloud"
                                },
                                |this, _, cx| {
                                    this.send(Request::SetProfile(SetupProfile::Full), cx);
                                    this.state.notice = Some("Setup profile set to Full.".into());
                                },
                                cx,
                            )),
                    )
                    .child(div().text_xs().text_color(rgb(theme::FAINT)).child(
                        "Cloud and paid providers require explicit paid authorization per run.",
                    ));

                columns = columns.child(mode_card).child(consents);
                content = content.child(columns);
            }
        }

        // Left icon rail.
        let rail = div()
            .id("page-rail")
            .accessibility_id("page-rail")
            .role(Role::TabList)
            // A vertical tab list says so; assistive technology otherwise assumes horizontal.
            .aria_orientation(gpui::Orientation::Vertical)
            .w(px(84.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .py_5()
            .bg(rgb(theme::RAIL))
            .border_r_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .w(px(38.))
                    .h(px(38.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // The symbol itself, paper body with the hot-copper fold,
                        // as two tinted layers: GPUI rasterizes SVG with the
                        // element's text colour, so each layer gets its own.
                        div()
                            .w(px(30.))
                            .h(px(30.))
                            .relative()
                            .child(
                                div().absolute().top(px(0.)).left(px(0.)).child(
                                    gpui::svg()
                                        .path(assets::SYMBOL_BODY)
                                        .size(px(30.))
                                        .text_color(rgb(theme::PAPER)),
                                ),
                            )
                            .child(
                                div().absolute().top(px(0.)).left(px(0.)).child(
                                    gpui::svg()
                                        .path(assets::SYMBOL_FOLD)
                                        .size(px(30.))
                                        .text_color(rgb(theme::HOT_COPPER)),
                                ),
                            ),
                    ),
            )
            .child(div().h(px(10.)).w(px(1.)).bg(rgb(theme::BORDER)))
            .child(self.nav_button(
                "nav-projects",
                theme::Icon::Projects,
                "Projects",
                Page::Projects,
                self.state.page == Page::Projects,
                cx,
            ))
            .child(self.nav_button(
                "nav-intake",
                theme::Icon::Intake,
                "Intake",
                Page::Intake,
                self.state.page == Page::Intake,
                cx,
            ))
            .child(self.nav_button(
                "nav-analysis",
                theme::Icon::Analysis,
                "Analysis",
                Page::Workflow,
                self.state.page == Page::Workflow,
                cx,
            ))
            .child(self.nav_button(
                "nav-history",
                theme::Icon::History,
                "History",
                Page::History,
                self.state.page == Page::History,
                cx,
            ))
            .child(self.nav_button(
                "nav-report",
                theme::Icon::Report,
                "Report",
                Page::Report,
                self.state.page == Page::Report,
                cx,
            ))
            .child(self.nav_button(
                "nav-models",
                theme::Icon::Setup,
                "Models",
                Page::Models,
                self.state.page == Page::Models,
                cx,
            ))
            .child(self.nav_button(
                "nav-settings",
                theme::Icon::Setup,
                "Setup",
                Page::Settings,
                self.state.page == Page::Settings,
                cx,
            ))
            .child(div().flex_1())
            .child(
                div()
                    .id("quit")
                    .role(Role::Button)
                    .aria_label("Quit Patent Forge Lite")
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .w(px(60.))
                    .py_3()
                    .rounded_lg()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .bg(rgb(theme::RAIL))
                    .hover(|s| s.bg(rgb(0x2b1a1f)))
                    .child(theme::icon(theme::Icon::Quit, theme::MUTED))
                    .child(div().text_xs().text_color(rgb(theme::MUTED)).child("Quit"))
                    .on_click(cx.listener(|this, _, _, cx| this.quit(cx)))
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                            this.quit(cx);
                            cx.stop_propagation();
                        }
                    })),
            );

        // Top bar: identity + live mode + provider.
        let provider = self.state.run.as_ref().map(|view| {
            format!(
                "{:?} · {}",
                view.snapshot.input.provider.kind, view.snapshot.input.provider.model
            )
        });
        let mut top_right = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(self.mode_pill());
        if let Some(provider) = provider {
            top_right = top_right.child(theme::pill(provider, theme::MUTED, 0x1b3247));
        }
        let top_bar = div()
            .h(px(64.))
            .px_6()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(rgb(theme::RAIL))
            .border_b_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(theme::TEXT))
                                    .child("PatentForge"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(theme::HOT_COPPER))
                                    .child("LITE"),
                            ),
                    )
                    .child(div().text_xs().text_color(rgb(theme::FAINT)).child(format!(
                        "Standalone workspace · v{} · {}",
                        env!("CARGO_PKG_VERSION"),
                        platform_name()
                    ))),
            )
            .child(top_right);

        // First run: ask for Full or Lean before showing anything else. The choice is
        // recorded in settings, and Lean refuses a local runtime by design.
        let content = if self.first_run {
            let prompt = div()
                .id("first-run-prompt")
                .accessibility_id("first-run-prompt")
                .role(Role::Group)
                .aria_label("Choose a profile to continue")
                .flex()
                .flex_col()
                .gap_4()
                .max_w(px(760.))
                .child(self.page_header(
                    "Choose how this build runs",
                    "One installer, two profiles. Full offers your own runtime and the paid Cloud provider. Lean offers Cloud only and refuses a local runtime outright. You can change this later in Setup.",
                    None,
                ))
                .child(self.primary_button(
                    "choose-full",
                    "Use Full (local and Cloud)",
                    |this, _, cx| {
                        this.send(Request::SetProfile(SetupProfile::Full), cx);
                        this.first_run = false;
                    },
                    cx,
                ))
                .child(self.button(
                    "choose-lean",
                    "Use Lean (Cloud only)",
                    |this, _, cx| {
                        this.send(Request::SetProfile(SetupProfile::Lean), cx);
                        this.first_run = false;
                    },
                    cx,
                ));
            div().child(prompt)
        } else {
            content
        };

        let main = div()
            .id("main-content")
            .accessibility_id("main-content")
            // The page area is the panel the rail selects, not an anonymous group.
            .role(Role::TabPanel)
            .aria_label("Page content")
            .track_focus(&self.content_focus)
            .focusable()
            .tab_stop(false)
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .px_8()
            .py_6()
            .child(div().w_full().child(content));

        let notice = self.state.notice.clone().unwrap_or_else(|| {
            "Standalone workspace · no automatic inference, migration or runtime takeover".into()
        });
        let status_bar = div()
            .h(px(34.))
            .px_6()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .bg(rgb(theme::RAIL))
            .border_t_1()
            .border_color(rgb(theme::BORDER))
            .child(
                div()
                    .id("status-notice")
                    .accessibility_id("status-notice")
                    .role(Role::Status)
                    .aria_label(notice.clone())
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(theme::dot(if self.state.notice.is_some() {
                        theme::ACCENT
                    } else {
                        theme::FAINT
                    }))
                    .child(div().text_xs().text_color(rgb(theme::MUTED)).child(notice)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme::FAINT))
                    .child("Ctrl+S save · Ctrl+Q quit · Ctrl+1–6 pages"),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(theme::BG))
            .text_color(rgb(theme::TEXT))
            .font_family(assets::FONT_FAMILY)
            .on_action(cx.listener(|this, _: &SaveIntake, _, cx| {
                this.save(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &StartRun, window, cx| {
                this.state.navigate(Page::Workflow);
                this.focus_body(window, cx);
                this.start(cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavProjects, window, cx| {
                this.state.navigate(Page::Projects);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavIntake, window, cx| {
                this.state.navigate(Page::Intake);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavAnalysis, window, cx| {
                this.state.navigate(Page::Workflow);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavHistory, window, cx| {
                this.state.navigate(Page::History);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavReport, window, cx| {
                this.state.navigate(Page::Report);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &NavSettings, window, cx| {
                this.state.navigate(Page::Settings);
                this.focus_body(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|this, _: &Quit, _, cx| this.quit(cx)))
            .child(top_bar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .child(rail)
                    .child(main),
            )
            .child(status_bar)
    }
}

/// Inline runs for one paragraph, preserving bold emphasis without wrapping
/// markers in the visible text.
fn report_runs(text: &str) -> Div {
    let runs = crate::markdown::runs(text);
    // The common report shape is "**Label:** body". Render that as a bold run
    // followed by a width-constrained run so the body wraps inside the column.
    if runs.len() == 2 && runs[0].bold && runs[0].text.chars().count() <= 48 {
        return div()
            .flex()
            .flex_row()
            .gap_1()
            .w_full()
            .child(
                div()
                    .flex_shrink_0()
                    .text_sm()
                    .line_height(px(24.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::DOC_TEXT))
                    .child(runs[0].text.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .line_height(px(24.))
                    .text_color(rgb(theme::DOC_TEXT))
                    .child(runs[1].text.clone()),
            );
    }
    let plain: String = runs.iter().map(|run| run.text.as_str()).collect();
    div()
        .w_full()
        .text_sm()
        .line_height(px(24.))
        .text_color(rgb(theme::DOC_TEXT))
        .child(plain)
}

fn report_table(header: Vec<String>, rows: Vec<Vec<String>>) -> Div {
    let mut table = div()
        .flex()
        .flex_col()
        .w_full()
        .rounded_md()
        .border_1()
        .border_color(rgb(theme::DOC_RULE))
        .overflow_hidden();
    if !header.is_empty() {
        table = table.child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .px_3()
                .py_2()
                .bg(rgb(0xefe9dd))
                .children(header.into_iter().map(|cell| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(theme::DOC_MUTED))
                        .child(cell)
                })),
        );
    }
    for row in rows {
        table = table.child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(rgb(theme::DOC_RULE))
                .children(row.into_iter().map(|cell| {
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(rgb(theme::DOC_TEXT))
                        .child(cell)
                })),
        );
    }
    table
}

/// Renders the saved report as a document: real heading hierarchy, emphasised
/// labels, tables and monospaced code instead of raw Markdown.
fn report_document(markdown_text: &str) -> Div {
    let mut column = div().flex().flex_col().gap_3().w_full();
    for block in crate::markdown::parse(markdown_text) {
        column = column.child(match block {
            crate::markdown::Block::Heading { level, text } => {
                let element = match level {
                    1 => div()
                        .text_2xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(theme::DOC_TEXT)),
                    2 => div()
                        .mt_2()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(theme::DOC_TEXT)),
                    _ => div()
                        .mt_2()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(theme::DOC_MUTED)),
                };
                element.child(text)
            }
            crate::markdown::Block::Paragraph(text) => report_runs(&text),
            crate::markdown::Block::Bullet(text) => div()
                .flex()
                .flex_row()
                .gap_2()
                .child(
                    div()
                        .mt(px(9.))
                        .w(px(5.))
                        .h(px(5.))
                        .rounded_full()
                        .bg(rgb(theme::DOC_COPPER)),
                )
                .child(report_runs(&text)),
            crate::markdown::Block::Numbered(text) => report_runs(&text),
            crate::markdown::Block::Rule => div().h(px(1.)).w_full().bg(rgb(theme::DOC_RULE)),
            crate::markdown::Block::Table { header, rows } => report_table(header, rows),
            crate::markdown::Block::Code(code) => div()
                .p_4()
                .rounded_md()
                .bg(rgb(theme::DOC_CODE))
                .border_1()
                .border_color(rgb(theme::DOC_RULE))
                .child(
                    div()
                        .text_xs()
                        .font_family("Consolas")
                        .line_height(px(18.))
                        .text_color(rgb(theme::DOC_MUTED))
                        .child(code),
                ),
        });
    }
    column
}
