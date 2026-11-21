// Ungated once the real storage/workflow commits are integrated into this worktree.
#![cfg(feature = "native")]

use async_trait::async_trait;
use pfl_desktop::session::{ApplicationSession, ExportFormat, Mode, Reply, Request, APPLICATION_ID, data_directory};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}},
    time::{Duration, Instant},
};

struct ControlledReplay {
    calls: AtomicUsize,
    block_second_once: AtomicBool,
}

impl ControlledReplay {
    fn new(block: bool) -> Self {
        Self { calls: AtomicUsize::new(0), block_second_once: AtomicBool::new(block) }
    }
}

#[async_trait]
impl Provider for ControlledReplay {
    fn snapshot(&self) -> ProviderSnapshot {
        ProviderSnapshot { kind: ProviderKind::Replay, model: "desktop-controlled-synthetic-v1".into(), endpoint: None }
    }
    async fn generate(&self, request: ProviderRequest, cancellation: CancellationToken,
        deltas: tokio::sync::mpsc::UnboundedSender<String>) -> Result<ProviderResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let text = format!("Synthetic stage {} café 世界", request.stage.number());
        let _ = deltas.send(text.clone());
        if request.stage == Stage::PriorArt && self.block_second_once.swap(false, Ordering::SeqCst) {
            cancellation.cancelled().await;
            return Err(Error::Cancelled);
        }
        Ok(ProviderResponse { text, usage: Usage { input_tokens: Some(4), output_tokens: Some(5) } })
    }
}

fn intake() -> Intake {
    Intake { title: "Synthetic desktop project".into(), description: "A synthetic test mechanism with no real invention data.".into(),
        technical_field: "Test devices".into(), context: InventionContext {
            additional_notes: "Keep optional café context".into(), ..Default::default()
        } }
}

fn until(session: &ApplicationSession, mut ready: impl FnMut(&Reply) -> bool) -> Reply {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(8) {
        for reply in session.drain() {
            if ready(&reply) { return reply; }
            if let Reply::Error(error) = reply { panic!("Unexpected application error: {error}"); }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("Application reply deadline exceeded");
}

fn wait_store(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(start.elapsed() < Duration::from_secs(8), "Store deadline exceeded");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn create(session: &ApplicationSession) -> Project {
    session.request(Request::Create(intake())).unwrap();
    match until(session, |r| matches!(r, Reply::Opened(_, _))) {
        Reply::Opened(project, _) => project, _ => unreachable!(),
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
    let session = ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    until(&session, |r| matches!(r, Reply::Projects(_)));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let project = create(&session);
    session.request(Request::Start { project: project.id.clone(), idempotency_key: "no-mode".into(),
        authorization: Authorization { synthetic_data: true, ..Default::default() } }).unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    session.request(Request::Start { project: project.id.clone(), idempotency_key: "no-consent".into(),
        authorization: Authorization::default() }).unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    for _ in 0..4 { session.request(Request::Open(project.id.clone())).unwrap(); }
    until(&session, |r| matches!(r, Reply::Opened(_, _)));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(store.history(&project.id).unwrap().is_empty());
    stop(&session);
}

#[test]
fn serialized_duplicate_start_navigation_cancel_and_resume_keep_one_owned_run() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ControlledReplay::new(true));
    let session = ApplicationSession::with_replay_provider(store.clone(), provider.clone()).unwrap();
    let project = create(&session);
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    for _ in 0..2 {
        session.request(Request::Start { project: project.id.clone(), idempotency_key: "same-start".into(),
            authorization: Authorization { synthetic_data: true, ..Default::default() } }).unwrap();
    }
    let run = match until(&session, |r| matches!(r, Reply::Run(_))) {
        Reply::Run(run) => run, _ => unreachable!(),
    };
    wait_store(|| store.run(&run.id).unwrap().checkpoints.len() == 1
        && provider.calls.load(Ordering::SeqCst) == 2);
    let checkpoint = store.run(&run.id).unwrap().checkpoints[0].clone();
    for _ in 0..3 { session.request(Request::Open(project.id.clone())).unwrap(); }
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
    assert_eq!(finished.input.intake.context.additional_notes, "Keep optional café context");
    stop(&session);
}

#[test]
fn imported_history_selection_exports_that_version_while_newer_run_completes() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("synthetic-legacy.sqlite3");
    pfl_storage::write_legacy_synthetic_fixture(&source).unwrap();
    let receipt = pfl_storage::migrate_legacy_synthetic(&source, &temp.path().join("imported-lite")).unwrap();
    let store = Arc::new(SqliteStore::open(receipt.activated_path.as_ref().unwrap()).unwrap());
    let session = ApplicationSession::with_store(store.clone()).unwrap();
    let project = ProjectId("synthetic-project".into());
    let older = RunId("synthetic-run-v1".into());
    session.request(Request::Open(project.clone())).unwrap();
    until(&session, |r| matches!(r, Reply::Opened(_, _)));
    session.request(Request::Configure(Mode::Replay)).unwrap();
    until(&session, |r| matches!(r, Reply::Configured(_)));
    session.request(Request::Start { project: project.clone(), idempotency_key: "new-version".into(),
        authorization: Authorization { synthetic_data: true, ..Default::default() } }).unwrap();
    let newest = match until(&session, |r| matches!(r, Reply::Run(run) if run.version == 3)) {
        Reply::Run(run) => run, _ => unreachable!(),
    };
    session.request(Request::SelectRun(older.clone())).unwrap();
    until(&session, |r| matches!(r, Reply::Run(run) if run.id == older));
    let destination = temp.path().join("selected-v1.md");
    session.request(Request::Export { run: older.clone(), path: destination.clone(), format: ExportFormat::Markdown }).unwrap();
    until(&session, |r| matches!(r, Reply::Exported(_)));
    let output = std::fs::read_to_string(&destination).unwrap();
    assert!(output.contains("Edited café report"));
    wait_store(|| store.run(&newest.id).unwrap().outcome == RunOutcome::Completed);
    assert_eq!(store.history(&project).unwrap().len(), 3);
    for reply in session.drain() {
        if let Reply::Poll { snapshot, .. } = reply { assert_eq!(snapshot.id, older); }
    }
    session.request(Request::Export { run: older, path: destination.clone(), format: ExportFormat::Html }).unwrap();
    until(&session, |r| matches!(r, Reply::Error(_)));
    assert_eq!(std::fs::read_to_string(destination).unwrap(), output, "Exports must not overwrite existing files");
    stop(&session);
}

#[test]
fn data_namespace_is_distinct_and_second_owner_is_rejected_without_stale_sentinel() {
    assert_eq!(APPLICATION_ID, "com.scottconverse.patentforgelite");
    assert_eq!(data_directory().unwrap().file_name().unwrap(), "patent-forge-lite");
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("lite.sqlite3");
    let session = ApplicationSession::open(&database).unwrap();
    assert!(matches!(ApplicationSession::open(&database), Err(Error::Conflict(_))));
    stop(&session);
    drop(session);
    let reopened = ApplicationSession::open(database).unwrap();
    stop(&reopened);
}

#[test]
fn lock_child_hold() {
    let Some(database) = std::env::var_os("PFL_SYNTHETIC_LOCK_CHILD_DATABASE") else { return; };
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
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
        .spawn().unwrap();
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
    assert!(rejected, "A second owner must not recover the active profile");
    let reopened = ApplicationSession::open(database).unwrap();
    stop(&reopened);
}
