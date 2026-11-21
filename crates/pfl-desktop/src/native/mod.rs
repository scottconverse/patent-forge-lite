//! GPUI desktop. Provider execution is exclusively owned by WorkflowService.
mod input;

use crate::{
    editor::TextBuffer,
    session::{
        APPLICATION_ID, APPLICATION_NAME, ApplicationSession, ExportFormat, Mode, Reply, Request,
        data_directory,
    },
    state::{DesktopState, EventApplied, Page, context_from_values},
};
use gpui::{
    App, Bounds, Context, Entity, KeyBinding, SharedString, Window, WindowBounds, WindowOptions,
    actions, div, prelude::*, px, rgb, size,
};
use input::TextInput;
use pfl_domain::*;
use std::{rc::Rc, time::Duration};

actions!(pfl_desktop, [SaveIntake, FocusNext, FocusPrevious, Quit]);

pub fn run() -> Result<()> {
    let replay = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [flag] if flag == "--replay" => true,
        [flag] if flag == "--help" || flag == "-h" => {
            println!(
                "Patent Forge Lite\n  --replay  explicitly select synthetic recorded responses (does not start a run)"
            );
            return Ok(());
        }
        _ => return Err(Error::Invalid("Supported options: --replay, --help".into())),
    };
    let directory = data_directory()?;
    std::fs::create_dir_all(&directory).map_err(|e| Error::Io(e.to_string()))?;
    let session = Rc::new(ApplicationSession::open(
        directory.join("projects.sqlite3"),
    )?);
    gpui_platform::application().run(move |cx: &mut App| {
        cx.set_app_identity(APPLICATION_ID, APPLICATION_NAME);
        input::bind_keys(cx);
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrevious, None),
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
                cx.new(|cx| Desktop::new(session.clone(), replay, window, cx))
            },
        );
        match result {
            Ok(_) => cx.activate(true),
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
    model: Entity<TextInput>,
    saved_intake: Intake,
    mode: Option<Mode>,
    workflow: WorkflowKind,
    synthetic: bool,
    authorized_local: bool,
    pending: bool,
    start_key: Option<String>,
    pending_open: Option<ProjectId>,
    confirm_close: bool,
    allow_close: bool,
}

impl Desktop {
    fn new(
        session: Rc<ApplicationSession>,
        replay: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| TextInput::new(TextBuffer::new("", false), 42., cx));
        let description = cx.new(|cx| TextInput::new(TextBuffer::new("", true), 250., cx));
        let field = cx.new(|cx| TextInput::new(TextBuffer::new("", false), 42., cx));
        let context = std::array::from_fn(|_| {
            cx.new(|cx| TextInput::new(TextBuffer::new("", true), 110., cx))
        });
        let report =
            cx.new(|cx| TextInput::new(TextBuffer::read_only("Select a saved report."), 500., cx));
        let stream =
            cx.new(|cx| TextInput::new(TextBuffer::read_only("No analysis running."), 390., cx));
        let endpoint =
            cx.new(|cx| TextInput::new(TextBuffer::new("http://127.0.0.1:11434", false), 42., cx));
        let model = cx.new(|cx| TextInput::new(TextBuffer::new("", false), 42., cx));
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
            model,
            saved_intake: Intake::default(),
            mode: None,
            workflow: WorkflowKind::Feasibility,
            synthetic: false,
            authorized_local: false,
            pending: false,
            start_key: None,
            pending_open: None,
            confirm_close: false,
            allow_close: false,
        }
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
                Reply::Projects(projects) => self.state.projects = projects,
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
                    self.state.show_run(snapshot);
                    self.pending = false;
                    self.start_key = None;
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
                    self.state.notice = Some(error);
                    self.pending = false;
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
        if self.pending {
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

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        if self.dirty(cx) {
            self.state.notice = Some("Save the current intake before starting a run.".into());
            return;
        }
        let Some(project) = &self.state.selected_project else {
            self.state.notice = Some("Create and save a project first.".into());
            return;
        };
        if self.state.run.as_ref().is_some_and(|r| {
            matches!(
                r.snapshot.outcome,
                RunOutcome::Running | RunOutcome::Cancelling
            )
        }) {
            self.state.notice = Some("The current run is still active.".into());
            return;
        }
        if !self.synthetic
            || (matches!(self.mode, Some(Mode::Local { .. })) && !self.authorized_local)
        {
            self.state.notice = Some(
                "Confirm synthetic data and any local-inference authorization in Execution setup."
                    .into(),
            );
            return;
        }
        let key = self
            .start_key
            .get_or_insert_with(|| RunId::new().to_string())
            .clone();
        let request = Request::Start {
            project: project.id.clone(),
            kind: self.workflow,
            idempotency_key: key,
            authorization: Authorization {
                synthetic_data: self.synthetic,
                local_inference: self.authorized_local,
                ..Default::default()
            },
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

    fn button(
        &self,
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        action: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let action = Rc::new(action);
        let keyboard_action = action.clone();
        div()
            .id(id)
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .px_3()
            .py_2()
            .rounded_md()
            .bg(rgb(0x253b52))
            .text_color(rgb(0xe4edf7))
            .text_sm()
            .hover(|s| s.bg(rgb(0x355574)))
            .child(label.into())
            .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                        keyboard_action(this, window, cx);
                        cx.stop_propagation();
                    }
                }),
            )
    }
}

impl Render for Desktop {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div().flex().flex_col().gap_3().w_full();
        match self.state.page {
            Page::Projects => {
                content = content.child(div().text_2xl().child("Projects"))
                    .child("Your separate Patent Forge Lite workspace. No legacy projects are imported automatically.")
                    .child(self.button("new-project", "New project", |this, _, cx| {
                        if this.dirty(cx) {
                            this.state.notice = Some("Save your current draft before creating another project.".into());
                        } else {
                            this.state.selected_project = None; this.state.run = None; this.state.history.clear();
                            this.load_intake(&Intake::default(), cx); this.state.page = Page::Intake;
                        }
                        cx.notify();
                    }, cx));
                for project in self.state.projects.clone() {
                    let id = project.id.clone();
                    content = content.child(self.button(
                        SharedString::from(format!("project-{}", project.id)),
                        project.intake.title,
                        move |this, _, cx| {
                            if this.dirty(cx) {
                                this.pending_open = Some(id.clone());
                                cx.notify();
                            } else {
                                this.send(Request::Open(id.clone()), cx);
                            }
                        },
                        cx,
                    ));
                }
                if self.state.projects.is_empty() {
                    content = content
                        .child("No projects yet. Create one to begin; startup never runs a model.");
                }
            }
            Page::Intake => {
                content = content
                    .child(div().text_2xl().child("Invention intake"))
                    .child("Title")
                    .child(self.title.clone())
                    .child("Technical field")
                    .child(self.field.clone())
                    .child("Description")
                    .child(self.description.clone())
                    .child(self.button(
                        "advanced-context",
                        if self.advanced {
                            "Hide optional invention context (values retained)"
                        } else {
                            "Edit optional invention context (9 fields)"
                        },
                        |this, _, cx| {
                            this.advanced = !this.advanced;
                            cx.notify();
                        },
                        cx,
                    ));
                if self.advanced {
                    for (index, (label, _)) in InventionContext::default()
                        .labeled_fields()
                        .into_iter()
                        .enumerate()
                    {
                        content = content.child(label).child(self.context[index].clone());
                    }
                }
                content = content.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(self.button(
                            "save-intake",
                            "Save intake",
                            |this, _, cx| this.save(cx),
                            cx,
                        ))
                        .child(self.button(
                            "start-intake",
                            "Start a new analysis",
                            |this, _, cx| this.start(cx),
                            cx,
                        )),
                );
            }
            Page::Workflow => {
                content = content.child(div().text_2xl().child(self.workflow.label()));
                content =
                    content.child(div().flex().gap_3().children(WorkflowKind::ALL.map(|kind| {
                        let id = SharedString::from(format!("workflow-{}", kind.slug()));
                        let selected = kind == self.workflow;
                        self.button(
                            id,
                            if selected {
                                format!("• {}", kind.label())
                            } else {
                                kind.label().to_string()
                            },
                            move |this, _, cx| {
                                this.workflow = kind;
                                this.start_key = None;
                                cx.notify();
                            },
                            cx,
                        )
                    })));
                content = content.child(format!(
                    "{} stages. A workflow that consumes another one needs that analysis complete first.",
                    self.workflow.stage_count()
                ));
                if let Some(view) = &self.state.run {
                    content = content
                        .child(format!(
                            "Version {} · {:?} · {} / {} checkpoints · event {}",
                            view.snapshot.version,
                            view.snapshot.outcome,
                            view.snapshot.checkpoints.len(),
                            view.snapshot.input.kind.stage_count(),
                            view.sequence
                        ))
                        .child(format!(
                            "Provider: {:?} · {}",
                            view.snapshot.input.provider.kind, view.snapshot.input.provider.model
                        ));
                    if let Some(message) = &view.message {
                        content = content.child(message.clone());
                    }
                    let id = view.snapshot.id.clone();
                    if view.can_cancel() {
                        content = content.child(self.button(
                            "cancel-run",
                            "Cancel run",
                            move |this, _, cx| {
                                this.send(Request::Cancel(id.clone()), cx);
                            },
                            cx,
                        ));
                    } else if view.can_resume() {
                        content = content.child(self.button("resume-run", "Resume this run", move |this, _, cx| {
                            if !this.synthetic || (matches!(this.mode, Some(Mode::Local { .. })) && !this.authorized_local) {
                                this.state.notice = Some("Confirm synthetic data and local authorization in Execution setup before resuming.".into());
                                cx.notify(); return;
                            }
                            this.send(Request::Resume(id.clone()), cx);
                        }, cx));
                    }
                } else {
                    content = content.child("No selected run. Save intake, configure execution and explicitly start an analysis.");
                }
                if self.state.run.is_some() {
                    content = content.child(self.stream.clone());
                }
                content = content.child(self.button(
                    "new-run",
                    "Start a new version",
                    |this, _, cx| this.start(cx),
                    cx,
                ));
            }
            Page::Report => {
                content = content.child(div().text_2xl().child("Saved report"))
                    .child("Research assistance, not legal advice. Review with a qualified patent professional.");
                if let Some(view) = &self.state.run {
                    content = content.child(format!(
                        "Selected version {} · {}",
                        view.snapshot.version, view.snapshot.id
                    ));
                }
                if self.state.run.is_some() {
                    content = content.child(self.report.clone());
                } else {
                    content = content.child(
                        "No saved report selected. Open a project or choose a version in History.",
                    );
                }
                content = content.child(
                    div()
                        .flex()
                        .gap_3()
                        .child(self.button(
                            "export-md",
                            "Export Markdown",
                            |this, _, cx| this.export(ExportFormat::Markdown, cx),
                            cx,
                        ))
                        .child(self.button(
                            "export-html",
                            "Export HTML",
                            |this, _, cx| this.export(ExportFormat::Html, cx),
                            cx,
                        ))
                        .child(self.button(
                            "export-word",
                            "Export Word",
                            |this, _, cx| this.export(ExportFormat::Word, cx),
                            cx,
                        )),
                );
            }
            Page::History => {
                content = content.child(div().text_2xl().child("Saved analysis history"));
                for snapshot in self.state.history.clone() {
                    let id = snapshot.id.clone();
                    content = content.child(self.button(
                        SharedString::from(format!("history-{id}")),
                        format!(
                            "Version {} · {:?} · {:?}",
                            snapshot.version, snapshot.outcome, snapshot.input.provider.kind
                        ),
                        move |this, _, cx| {
                            this.state.page = Page::Report;
                            this.send(Request::SelectRun(id.clone()), cx);
                        },
                        cx,
                    ));
                }
                if self.state.history.is_empty() {
                    content = content
                        .child("No saved runs yet. Reopen the project to refresh its history.");
                }
            }
            Page::Settings => {
                content = content.child(div().text_2xl().child("Execution setup"))
                    .child("Choose explicitly. Configuration does not send a request, start Ollama or pull a model.")
                    .child(self.button("replay-mode", "Use synthetic recorded replay", |this, _, cx| {
                        this.send(Request::Configure(Mode::Replay), cx);
                    }, cx))
                    .child("Local Ollama endpoint (on-machine only for this slice)").child(self.endpoint.clone())
                    .child("Already-installed model name").child(self.model.clone())
                    .child(self.button("local-mode", "Configure existing local runtime", |this, _, cx| {
                        let mode = Mode::Local { endpoint: this.endpoint.read(cx).buffer.text().into(),
                            model: this.model.read(cx).buffer.text().into() };
                        this.send(Request::Configure(mode), cx);
                    }, cx))
                    .child(self.button("synthetic-consent",
                        if self.synthetic { "[x] This invention is synthetic test data" } else { "[ ] This invention is synthetic test data" },
                        |this, _, cx| { this.synthetic = !this.synthetic; cx.notify(); }, cx))
                    .child(self.button("local-consent",
                        if self.authorized_local { "[x] Authorize local inference for this synthetic input" } else { "[ ] Authorize local inference for this synthetic input" },
                        |this, _, cx| { this.authorized_local = !this.authorized_local; cx.notify(); }, cx))
                    .child("Cloud and paid providers are fail-closed in this first slice. Full/Lean distribution profiles are later parity work.");
            }
        }

        let mut navigation = div()
            .w(px(205.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(0x142436));
        for (id, name, page) in [
            ("nav-projects", "Projects", Page::Projects),
            ("nav-intake", "Intake", Page::Intake),
            ("nav-analysis", "Analysis", Page::Workflow),
            ("nav-history", "History", Page::History),
            ("nav-report", "Report", Page::Report),
            ("nav-settings", "Execution setup", Page::Settings),
        ] {
            navigation = navigation.child(self.button(
                id,
                name,
                move |this, _, cx| {
                    this.state.navigate(page);
                    // Navigation is read-only: don't start, resume or reconstruct a runner.
                    cx.notify();
                },
                cx,
            ));
        }
        navigation = navigation.child(self.button("quit", "Quit", |this, _, cx| this.quit(cx), cx));
        let mode = match &self.mode {
            None => "Not configured · no inference",
            Some(Mode::Replay) => "RECORDED REPLAY · SYNTHETIC ONLY",
            Some(Mode::Local { .. }) => "Local inference · explicit authorization required",
        };
        let mut main = div()
            .id("main-content")
            .flex_1()
            .min_w_0()
            .p_6()
            .overflow_y_scroll()
            .child(content);
        if let Some(id) = self.pending_open.clone() {
            main = main.child(
                div()
                    .mt_4()
                    .p_3()
                    .bg(rgb(0x594328))
                    .child("Unsaved intake edits. Discard them and open the selected project?")
                    .child(self.button(
                        "discard-open",
                        "Discard edits and open",
                        move |this, _, cx| {
                            this.pending_open = None;
                            this.send(Request::Open(id.clone()), cx);
                        },
                        cx,
                    ))
                    .child(self.button(
                        "keep-draft",
                        "Keep editing",
                        |this, _, cx| {
                            this.pending_open = None;
                            cx.notify();
                        },
                        cx,
                    )),
            );
        }
        if self.confirm_close {
            main = main.child(div().mt_4().p_3().bg(rgb(0x594328))
                .child("Quit now? Unsaved intake edits will be discarded. Every in-progress analysis this app owns, in any project, will be interrupted for explicit recovery/resume.")
                .child(self.button("confirm-quit", "Quit without saving", |this, _, cx| {
                    this.allow_close = true; cx.quit();
                }, cx))
                .child(self.button("cancel-quit", "Keep working", |this, _, cx| {
                    this.confirm_close = false; cx.notify();
                }, cx)));
        }
        div().size_full().flex().flex_col().bg(rgb(0x0d1723)).text_color(rgb(0xdde7f3))
            .on_action(cx.listener(|this, _: &SaveIntake, _, cx| this.save(cx)))
            .on_action(cx.listener(|_, _: &FocusNext, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevious, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|this, _: &Quit, _, cx| this.quit(cx)))
            .child(div().p_4().border_b_1().border_color(rgb(0x30465d))
                .child(div().text_xl().child(APPLICATION_NAME)).child(mode))
            .child(div().flex().flex_1().min_h_0().child(navigation).child(main))
            .child(div().px_4().py_2().bg(rgb(0x1b2d41)).child(
                self.state.notice.clone().unwrap_or_else(|| "Standalone workspace · no automatic inference, migration or runtime takeover".into())
            ))
    }
}
