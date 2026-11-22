// Headless actor tests: real storage/workflow peers, no GPU or native feature required.

use async_trait::async_trait;
use pfl_desktop::session::{
    APPLICATION_ID, ApplicationSession, ExportFormat, Mode, Reply, Request, data_directory,
};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

struct ControlledReplay {
    calls: AtomicUsize,
    block_second_once: AtomicBool,
}

impl ControlledReplay {
    fn new(block: bool) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            block_second_once: AtomicBool::new(block),
        }
    }
}

#[async_trait]
impl Provider for ControlledReplay {
    fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot {
            kind: ProviderKind::Replay,
            model: "desktop-controlled-synthetic-v1".into(),
            endpoint: None,
        }
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: tokio::sync::mpsc::UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let text = format!("Synthetic stage {} café 世界", request.stage.number());
        let _ = deltas.send(text.clone());
        if request.stage == Stage::PriorArt && self.block_second_once.swap(false, Ordering::SeqCst)
        {
            cancellation.cancelled().await;
            return Err(Error::Cancelled);
        }
        Ok(ProviderResponse {
            text,
            usage: Usage {
                input_tokens: Some(4),
                output_tokens: Some(5),
            },
        })
    }
}

fn intake() -> Intake {
    Intake {
        title: "Synthetic desktop project".into(),
        description: concat!(
            "This synthetic test mechanism exists only to exercise the desktop actor and ",
            "contains no real invention data. It describes a hypothetical device with a ",
            "clamped frame, a rotating sample holder, and a sensor that records displacement ",
            "over time, so that replay fixtures can produce deterministic text for each of ",
            "the six feasibility stages without contacting any provider or model."
        )
        .into(),
        technical_field: "Test devices".into(),
        context: InventionContext {
            additional_notes: "Keep optional café context".into(),
            ..Default::default()
        },
    }
}

fn until(session: &ApplicationSession, mut ready: impl FnMut(&Reply) -> bool) -> Reply {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        for reply in session.drain() {
            if ready(&reply) {
                return reply;
            }
            if let Reply::Error(error) = reply {
                panic!("Unexpected application error: {error}");
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Application reply deadline exceeded");
}

fn wait_store(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(8),
            "Store deadline exceeded"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn create(session: &ApplicationSession) -> Project {
    session.request(Request::Create(intake())).unwrap();
    match until(session, |r| matches!(r, Reply::Opened(_, _))) {
        Reply::Opened(project, _) => project,
        _ => unreachable!(),
    }
}

fn stop(session: &ApplicationSession) {
    session.request(Request::Shutdown).unwrap();
    until(session, |r| matches!(r, Reply::Stopped));
}

#[test]
fn construction_open_configure_and_repeated_reads_send_zero_inference_calls() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session =
        ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let project = create(&session);
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "no-mode".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "no-consent".into(),
            authorization: Authorization::default(),
        })
        .unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    for _ in 0..4 {
        session.request(Request::Open(project.id.clone())).unwrap();
    }
    until(&session, |r| matches!(r, Reply::Opened(_, _)));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(store.history(&project.id).unwrap().is_empty());
    stop(&session);
}

#[test]
fn serialized_duplicate_start_navigation_cancel_and_resume_keep_one_owned_run() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(true));
    let session =
        ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    let project = create(&session);
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    for _ in 0..2 {
        session
            .request(Request::Start {
                project: project.id.clone(),
                kind: WorkflowKind::Feasibility,
                idempotency_key: "same-start".into(),
                authorization: Authorization {
                    synthetic_data: true,
                    ..Default::default()
                },
            })
            .unwrap();
    }
    let run = match until(&session, |r| matches!(r, Reply::Run(_))) {
        Reply::Run(run) => run,
        _ => unreachable!(),
    };
    wait_store(|| {
        store.run(&run.id).unwrap().checkpoints.len() == 1
            && provider.calls.load(Ordering::SeqCst) == 2
    });
    let checkpoint = store.run(&run.id).unwrap().checkpoints[0].clone();
    for _ in 0..3 {
        session.request(Request::Open(project.id.clone())).unwrap();
    }
    until(&session, |r| matches!(r, Reply::Opened(_, _)));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert_eq!(store.history(&project.id).unwrap().len(), 1);
    session.request(Request::Cancel(run.id.clone())).unwrap();
    wait_store(|| store.run(&run.id).unwrap().outcome == RunOutcome::Cancelled);
    session.request(Request::Resume(run.id.clone())).unwrap();
    wait_store(|| store.run(&run.id).unwrap().outcome == RunOutcome::Completed);
    let finished = store.run(&run.id).unwrap();
    assert_eq!(finished.checkpoints.len(), 6);
    assert_eq!(finished.checkpoints[0], checkpoint);
    assert_eq!(finished.attempts.len(), 2);
    assert_eq!(provider.calls.load(Ordering::SeqCst), 7);
    assert_eq!(
        finished.input.intake.context.additional_notes,
        "Keep optional café context"
    );
    stop(&session);
}

#[test]
fn imported_history_selection_exports_that_version_while_newer_run_completes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic-legacy.sqlite3");
    pfl_storage::write_legacy_synthetic_fixture(&source).unwrap();
    let receipt =
        pfl_storage::migrate_legacy_synthetic(&source, &temp.path().join("imported-lite")).unwrap();
    let store = Arc::new(SqliteStore::open(receipt.activated_path.as_ref().unwrap()).unwrap());
    let session = ApplicationSession::with_store(store.clone()).unwrap();
    let project = ProjectId("synthetic-project".into());
    let older = RunId("synthetic-run-v1".into());
    session.request(Request::Open(project.clone())).unwrap();
    until(&session, |r| matches!(r, Reply::Opened(_, _)));
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    // The imported fixture description is deliberately below the explicit word
    // minimum, so a new version requires an explicit edit of the CURRENT intake.
    // Every other imported field, and all historical snapshots, stay untouched.
    let mut edited = store.project(&project).unwrap().intake;
    edited.description = concat!(
        "Synthetic imported description edited only to meet the explicit word minimum ",
        "before starting a new analysis version, with no real invention data and no ",
        "other change. The hypothetical device still has a clamped frame, a rotating ",
        "sample holder, and a displacement sensor used solely to generate deterministic ",
        "replay text for the six feasibility stages in this headless desktop test."
    )
    .into();
    session
        .request(Request::Save(project.clone(), edited))
        .unwrap();
    until(&session, |r| matches!(r, Reply::Saved(_)));
    session
        .request(Request::Start {
            project: project.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "new-version".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let newest = match until(
        &session,
        |r| matches!(r, Reply::Run(run) if run.version == 3),
    ) {
        Reply::Run(run) => run,
        _ => unreachable!(),
    };
    session.request(Request::SelectRun(older.clone())).unwrap();
    until(
        &session,
        |r| matches!(r, Reply::Run(run) if run.id == older),
    );
    let destination = temp.path().join("selected-v1.md");
    session
        .request(Request::Export {
            run: older.clone(),
            path: destination.clone(),
            format: ExportFormat::Markdown,
        })
        .unwrap();
    until(&session, |r| matches!(r, Reply::Exported(_)));
    let output = std::fs::read_to_string(&destination).unwrap();
    assert!(output.contains("Edited café report"));
    wait_store(|| store.run(&newest.id).unwrap().outcome == RunOutcome::Completed);
    assert_eq!(store.history(&project).unwrap().len(), 3);
    for reply in session.drain() {
        if let Reply::Poll { snapshot, .. } = reply {
            assert_eq!(snapshot.id, older);
        }
    }
    session
        .request(Request::Export {
            run: older,
            path: destination.clone(),
            format: ExportFormat::Html,
        })
        .unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    assert_eq!(
        std::fs::read_to_string(destination).unwrap(),
        output,
        "Exports must not overwrite existing files"
    );
    stop(&session);
}

#[test]
fn data_namespace_is_distinct_and_second_owner_is_rejected_without_stale_sentinel() {
    assert_eq!(APPLICATION_ID, "com.scottconverse.patentforgelite");
    assert_eq!(
        data_directory().unwrap().file_name().unwrap(),
        "patent-forge-lite"
    );
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("lite.sqlite3");
    let session = ApplicationSession::open(&database).unwrap();
    assert!(matches!(
        ApplicationSession::open(&database),
        Err(Error::Conflict(_))
    ));
    stop(&session);
    drop(session);
    let reopened = ApplicationSession::open(database).unwrap();
    stop(&reopened);
}

#[test]
fn lock_child_hold() {
    let Some(database) = std::env::var_os("PFL_SYNTHETIC_LOCK_CHILD_DATABASE") else {
        return;
    };
    let database = PathBuf::from(database);
    let _session = ApplicationSession::open(&database).unwrap();
    std::fs::write(database.with_extension("ready"), b"synthetic lock held").unwrap();
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
fn process_death_releases_only_the_lite_profile_lock_and_allows_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("child-lite.sqlite3");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_child_hold", "--nocapture"])
        .env("PFL_SYNTHETIC_LOCK_CHILD_DATABASE", &database)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    while !database.with_extension("ready").exists() && start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(10));
    }
    let ready = database.with_extension("ready").exists();
    let rejected = matches!(ApplicationSession::open(&database), Err(Error::Conflict(_)));
    // This is our exact child process, not a platform-wide process-name kill.
    let _ = child.kill();
    let _ = child.wait();
    assert!(ready, "Owned synthetic child did not acquire its lock");
    assert!(
        rejected,
        "A second owner must not recover the active profile"
    );
    let reopened = ApplicationSession::open(database).unwrap();
    stop(&reopened);
}

#[test]
fn actor_runs_a_dependent_workflow_and_reports_unmet_prerequisites() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session =
        ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));
    let project = create(&session);
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "multi-feasibility".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let feasibility = match until(&session, |r| matches!(r, Reply::Run(_))) {
        Reply::Run(run) => run,
        _ => unreachable!(),
    };
    wait_store(|| store.run(&feasibility.id).unwrap().outcome.is_terminal());
    let feasibility = store.run(&feasibility.id).unwrap();
    assert_eq!(feasibility.outcome, RunOutcome::Completed);
    assert_eq!(feasibility.checkpoints.len(), 6);

    // A second workflow over the same project, driven through the same actor.
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Claims,
            idempotency_key: "multi-claims".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let claims = match until(&session, |r| matches!(r, Reply::Run(_))) {
        Reply::Run(run) => run,
        _ => unreachable!(),
    };
    wait_store(|| store.run(&claims.id).unwrap().outcome.is_terminal());
    let saved = store.run(&claims.id).unwrap();
    assert_eq!(saved.outcome, RunOutcome::Completed);
    assert_eq!(saved.input.kind, WorkflowKind::Claims);
    assert_eq!(saved.checkpoints.len(), 4);
    assert!(
        saved
            .report
            .clone()
            .unwrap()
            .markdown
            .contains("Synthetic stage 4"),
        "the revision pass is the saved claims report"
    );

    // A fresh project cannot start compliance with no claims to review.
    let fresh = create(&session);
    session
        .request(Request::Start {
            project: fresh.id.clone(),
            kind: WorkflowKind::Compliance,
            idempotency_key: "multi-compliance".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let error = match until(&session, |r| matches!(r, Reply::Error(_))) {
        Reply::Error(error) => error,
        _ => unreachable!(),
    };
    assert!(error.contains("must complete"), "unexpected error: {error}");
    assert_eq!(session.active_jobs(), 0);
    stop(&session);
}

#[test]
fn quit_guard_counts_jobs_owned_by_another_project_or_historical_run() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(true));
    let session =
        ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));
    let owner = create(&session);
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    session
        .request(Request::Start {
            project: owner.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "owned-elsewhere".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let running = match until(
        &session,
        |r| matches!(r, Reply::Run(run) if run.outcome == RunOutcome::Running),
    ) {
        Reply::Run(run) => run,
        _ => unreachable!(),
    };
    // The counter can lag the Run reply by a scheduling tick, so wait for it rather
    // than asserting immediately; this failed only on the macOS runner.
    wait_store(|| session.active_jobs() == 1);
    assert_eq!(session.active_jobs(), 1, "the started job must be counted");
    // Open an unrelated project that has no run. The window no longer shows the
    // active job, but the application still owns it and must still warn on quit.
    let _other = create(&session);
    assert_eq!(
        session.active_jobs(),
        1,
        "an unselected job is still app-owned"
    );
    session.request(Request::Cancel(running.id)).unwrap();
    let start = Instant::now();
    while session.active_jobs() != 0 && start.elapsed() < Duration::from_secs(8) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        session.active_jobs(),
        0,
        "a terminal job must stop counting"
    );
    stop(&session);
}

#[test]
fn cloud_without_a_key_is_refused_and_sends_nothing() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session = ApplicationSession::with_replay_provider(store, provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));

    // Configuring Cloud is where the missing key is caught: no provider is built,
    // no run exists and no request can leave the process.
    let configured = session.request(Request::Configure(Mode::Cloud {
        model: "claude-test".into(),
    }));
    assert!(configured.is_ok());
    match until(&session, |r| {
        matches!(r, Reply::Configured(_) | Reply::Error(_))
    }) {
        Reply::Error(message) => assert!(
            message.contains("API key"),
            "the refusal must name the missing key: {message}"
        ),
        other => panic!("Cloud without a key must be refused, got {other:?}"),
    }
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        0,
        "no provider call may happen while configuring"
    );
    stop(&session);
}

#[test]
fn cloud_start_requires_explicit_paid_approval() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session = ApplicationSession::with_replay_provider(store, provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));
    let project = create(&session);

    session
        .request(Request::SetCloudKey(Some("session-only-test-key".into())))
        .unwrap();
    until(&session, |r| matches!(r, Reply::Settings(_)));
    session
        .request(Request::Configure(Mode::Cloud {
            model: "claude-test".into(),
        }))
        .unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));

    // No paid approval: refused before any provider work.
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "cloud-unapproved".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    match until(&session, |r| matches!(r, Reply::Error(_) | Reply::Run(_))) {
        Reply::Error(message) => assert!(
            message.contains("paid Cloud"),
            "the refusal must name the missing approval: {message}"
        ),
        other => panic!("an unapproved Cloud run must be refused, got {other:?}"),
    }
    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        0,
        "a refused Cloud run must send nothing"
    );
    stop(&session);
}

/// Replies folded into state, so a test can wait for a condition instead of racing
/// individual replies. `until()` panics on any error it happens to drain, which made
/// multi-step flows unworkable.
#[derive(Default)]
struct Drained {
    projects: Vec<Project>,
    projects_seen: bool,
    opened: Option<Project>,
    latest: Option<RunSnapshot>,
    revisions: Vec<DocumentRevision>,
    configured: Option<Mode>,
    errors: Vec<String>,
    stopped: bool,
}

fn fold(session: &ApplicationSession, state: &mut Drained) {
    for reply in session.drain() {
        match reply {
            Reply::Projects(projects) => {
                state.projects_seen = true;
                state.projects = projects;
            }
            Reply::Opened(project, _runs) => state.opened = Some(project),
            Reply::Run(run) | Reply::Poll { snapshot: run, .. } => state.latest = Some(run),
            Reply::Revision(revision) => state.revisions.push(revision),
            Reply::Revisions(list) => state.revisions = list,
            Reply::Configured(mode) => state.configured = Some(mode),
            Reply::Error(error) => state.errors.push(error),
            Reply::Stopped => state.stopped = true,
            _ => {}
        }
    }
}

fn settle_into(
    session: &ApplicationSession,
    state: &mut Drained,
    label: &str,
    mut ready: impl FnMut(&Drained) -> bool,
) {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(25) {
        fold(session, state);
        if ready(state) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("deadline exceeded at {label}; errors: {:?}", state.errors);
}

fn terminal_of(state: &Drained, kind: WorkflowKind) -> bool {
    state
        .latest
        .as_ref()
        .map(|run| run.input.kind == kind && run.outcome.is_terminal())
        .unwrap_or(false)
}

#[test]
fn editing_a_completed_run_saves_numbered_revisions_and_lists_them() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session = ApplicationSession::with_replay_provider(store, provider).unwrap();
    let mut state = Drained::default();
    settle_into(&session, &mut state, "projects", |s| s.projects_seen);
    let project = create(&session);

    session.request(Request::Configure(Mode::Replay)).unwrap();
    settle_into(&session, &mut state, "configure", |s| {
        s.configured.is_some()
    });

    let authorization = Authorization {
        synthetic_data: true,
        ..Default::default()
    };
    for (kind, key) in [
        (WorkflowKind::Feasibility, "b1-feasibility"),
        (WorkflowKind::Claims, "b1-claims"),
    ] {
        state.latest = None;
        session
            .request(Request::Start {
                project: project.id.clone(),
                kind,
                idempotency_key: key.into(),
                authorization: authorization.clone(),
            })
            .unwrap();
        settle_into(&session, &mut state, "run terminal", |s| {
            terminal_of(s, kind)
        });
        assert_eq!(
            state.latest.as_ref().unwrap().outcome,
            RunOutcome::Completed,
            "{kind:?} must complete in replay"
        );
    }
    let claims_run = state.latest.as_ref().unwrap().id.clone();

    // First edit: version 1, anchored to the completed claims run.
    session
        .request(Request::SaveRevision {
            project: project.id.clone(),
            kind: DocumentKind::Claims,
            text: "1. An edited claim.".into(),
            note: "first edit".into(),
        })
        .unwrap();
    settle_into(&session, &mut state, "first revision", |s| {
        !s.revisions.is_empty()
    });
    assert_eq!(state.revisions[0].version, 1);
    assert_eq!(state.revisions[0].base_run_id.as_ref(), Some(&claims_run));

    // Second edit: version 2 of the same kind.
    session
        .request(Request::SaveRevision {
            project: project.id.clone(),
            kind: DocumentKind::Claims,
            text: "1. A further edited claim.".into(),
            note: "second edit".into(),
        })
        .unwrap();
    settle_into(&session, &mut state, "second revision", |s| {
        s.revisions.iter().any(|revision| revision.version == 2)
    });

    // Listing returns the stored series in order.
    session
        .request(Request::ListRevisions {
            project: project.id.clone(),
            kind: DocumentKind::Claims,
        })
        .unwrap();
    settle_into(&session, &mut state, "list revisions", |s| {
        s.revisions.len() == 2
    });
    assert_eq!(state.revisions[0].version, 1);
    assert_eq!(state.revisions[1].version, 2);
    assert_eq!(state.revisions[1].text, "1. A further edited claim.");

    // An empty edit is refused rather than stored.
    session
        .request(Request::SaveRevision {
            project: project.id.clone(),
            kind: DocumentKind::Claims,
            text: "   ".into(),
            note: String::new(),
        })
        .unwrap();
    settle_into(&session, &mut state, "empty edit refused", |s| {
        s.errors.iter().any(|error| error.contains("needs text"))
    });

    stop(&session);
}

#[test]
fn compliance_started_with_a_selected_revision_records_it() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(false));
    let session = ApplicationSession::with_replay_provider(store, provider).unwrap();
    let mut state = Drained::default();
    settle_into(&session, &mut state, "projects", |s| s.projects_seen);
    let project = create(&session);
    session.request(Request::Configure(Mode::Replay)).unwrap();
    settle_into(&session, &mut state, "configure", |s| {
        s.configured.is_some()
    });

    let authorization = Authorization {
        synthetic_data: true,
        ..Default::default()
    };
    for (kind, key) in [
        (WorkflowKind::Feasibility, "sel-feasibility"),
        (WorkflowKind::Claims, "sel-claims"),
    ] {
        state.latest = None;
        session
            .request(Request::Start {
                project: project.id.clone(),
                kind,
                idempotency_key: key.into(),
                authorization: authorization.clone(),
            })
            .unwrap();
        settle_into(&session, &mut state, "run terminal", |s| {
            terminal_of(s, kind)
        });
    }

    session
        .request(Request::SaveRevision {
            project: project.id.clone(),
            kind: DocumentKind::Claims,
            text: "1. THE SELECTED CLAIM SET.".into(),
            note: "for review".into(),
        })
        .unwrap();
    settle_into(&session, &mut state, "revision", |s| {
        !s.revisions.is_empty()
    });
    let selected = state.revisions[0].id.clone();

    state.latest = None;
    session
        .request(Request::StartSelected {
            project: project.id.clone(),
            kind: WorkflowKind::Compliance,
            idempotency_key: "sel-compliance".into(),
            authorization,
            claims_revision: Some(selected.clone()),
            specification_revision: None,
        })
        .unwrap();
    settle_into(&session, &mut state, "compliance terminal", |s| {
        terminal_of(s, WorkflowKind::Compliance)
    });
    let run = state.latest.as_ref().expect("compliance run");
    assert_eq!(run.outcome, RunOutcome::Completed);
    assert_eq!(
        run.input.upstream.claims_revision_id.as_ref(),
        Some(&selected),
        "the selected claims revision is recorded in the run"
    );
    assert_eq!(run.input.upstream.claims_text, "1. THE SELECTED CLAIM SET.");

    stop(&session);
}
