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
