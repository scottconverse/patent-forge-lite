use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use tokio::{runtime::Handle, sync::Notify};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Point {
    History,
    Create,
    Begin,
}

struct BarrierStore {
    inner: SqliteStore,
    point: Point,
    armed: AtomicBool,
    entered: Notify,
    released: (Mutex<bool>, Condvar),
}
impl BarrierStore {
    fn pause(&self, point: Point) {
        if self.point != point || !self.armed.swap(false, Ordering::SeqCst) {
            return;
        }
        self.entered.notify_one();
        let mut released = self.released.0.lock().unwrap();
        while !*released {
            released = self.released.1.wait(released).unwrap();
        }
    }
    fn release(&self) {
        *self.released.0.lock().unwrap() = true;
        self.released.1.notify_all();
    }
}
macro_rules! forward {
    ($method:ident($($arg:ident : $ty:ty),*) -> $ret:ty) => {
        fn $method(&self, $($arg: $ty),*) -> $ret { self.inner.$method($($arg),*) }
    };
}
impl Store for BarrierStore {
    forward!(create_project(project: &Project) -> Result<Project>);
    forward!(list_projects() -> Result<Vec<Project>>);
    forward!(project(id: &ProjectId) -> Result<Project>);
    forward!(save_intake(id: &ProjectId, intake: &Intake, at_ms: u64) -> Result<Project>);
    forward!(save_revision(revision: &DocumentRevision) -> Result<DocumentRevision>);
    forward!(revisions(project: &ProjectId, kind: DocumentKind) -> Result<Vec<DocumentRevision>>);
    forward!(revision(id: &RevisionId) -> Result<DocumentRevision>);
    forward!(settings() -> Result<Settings>);
    forward!(save_settings(settings: &Settings) -> Result<()>);
    fn create_run(&self, request: &NewRun) -> Result<RunSnapshot> {
        self.pause(Point::Create);
        self.inner.create_run(request)
    }
    forward!(run(id: &RunId) -> Result<RunSnapshot>);
    fn history(&self, id: &ProjectId) -> Result<Vec<RunSnapshot>> {
        self.pause(Point::History);
        self.inner.history(id)
    }
    fn begin_attempt(&self, id: &RunId, at_ms: u64) -> Result<Attempt> {
        self.pause(Point::Begin);
        self.inner.begin_attempt(id, at_ms)
    }
    forward!(append_event(run: &RunId, attempt: &AttemptId, event: EventKind, at_ms: u64) -> Result<RunEvent>);
    forward!(commit_checkpoint(run: &RunId, attempt: &AttemptId, checkpoint: &Checkpoint) -> Result<RunEvent>);
    forward!(set_cancelling(run: &RunId, attempt: &AttemptId, at_ms: u64) -> Result<RunSnapshot>);
    forward!(finish_attempt(run: &RunId, attempt: &AttemptId, outcome: RunOutcome, report: Option<&SavedReport>, at_ms: u64) -> Result<RunSnapshot>);
    forward!(events_after(cursor: &Subscribe) -> Result<Vec<RunEvent>>);
    forward!(recover_interrupted(at_ms: u64) -> Result<Vec<RunId>>);
}

fn project() -> Project {
    Project {
        id: ProjectId::new(), created_at_ms: 1, updated_at_ms: 1,
        intake: Intake {
            title: "Synthetic shutdown barrier".into(),
            description: "This is a completely synthetic mechanical marker attached to a fictional tray and an imaginary docking station. A spring moves the indicator when the tray returns. This description is used only for deterministic lifecycle tests with recorded responses and a temporary database. It describes no real invention and makes no patentability claim.".into(),
            ..Default::default()
        },
    }
}
fn request(project: &Project, provider: &ReplayProvider, key: &str) -> NewRun {
    NewRun {
        id: RunId::new(),
        project_id: project.id.clone(),
        idempotency_key: key.into(),
        input: RunInput {
            kind: WorkflowKind::Feasibility,
            intake: project.intake.clone(),
            provider: provider.snapshot(),
            prompts: prompts::snapshot(WorkflowKind::Feasibility),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
        },
        rerun_of: None,
        created_at_ms: 1,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shutdown_serializes_inflight_start_resume_rerun_registration() {
    // Pauses occur inside actual synchronous Store calls while the facade's
    // asynchronous command is suspended. No synthetic timing assumption can
    // make registration finish before the test has attempted shutdown.
    for (route, point) in [
        ("start", Point::History),
        ("start", Point::Create),
        ("start", Point::Begin),
        ("resume", Point::Begin),
        ("rerun", Point::Begin),
    ] {
        let store = Arc::new(BarrierStore {
            inner: SqliteStore::in_memory().unwrap(),
            point,
            armed: AtomicBool::new(false),
            entered: Notify::new(),
            released: (Mutex::new(false), Condvar::new()),
        });
        let provider = Arc::new(ReplayProvider::synthetic().with_delay(Duration::from_millis(50)));
        let p = project();
        store.inner.create_project(&p).unwrap();
        let original = request(&p, &provider, "original");
        if route != "start" {
            store.inner.create_run(&original).unwrap();
            let attempt = store.inner.begin_attempt(&original.id, 2).unwrap();
            store
                .inner
                .finish_attempt(&original.id, &attempt.id, RunOutcome::Interrupted, None, 3)
                .unwrap();
        }
        let pending_request = if route == "rerun" {
            request(&p, &provider, "new-version")
        } else {
            original.clone()
        };
        let expected_run_id = pending_request.id.clone();
        store.armed.store(true, Ordering::SeqCst);
        let service = WorkflowService::new(store.clone(), provider.clone(), Handle::current());
        let starter = service.clone();
        let source = original.id.clone();
        let starting = tokio::spawn(async move {
            match route {
                "resume" => starter.resume(source).await,
                "rerun" => starter.rerun(source, pending_request).await,
                _ => starter.start(pending_request).await,
            }
        });
        tokio::time::timeout(Duration::from_secs(3), store.entered.notified())
            .await
            .unwrap();
        let stopper = service.clone();
        let mut stopping = tokio::spawn(async move { stopper.shutdown().await });
        // Without the admission gate this returns immediately with an empty
        // job map, while the start is still held at its deterministic barrier.
        let premature = tokio::time::timeout(Duration::from_millis(25), &mut stopping).await;
        store.release(); // Release even when the assertion is about to fail.
        assert!(
            premature.is_err(),
            "shutdown escaped an in-flight {route} registration"
        );
        starting.await.unwrap().unwrap();
        tokio::time::timeout(Duration::from_secs(3), stopping)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let calls_at_return = provider.request_count();
        let stopped = service.run(expected_run_id.clone()).await.unwrap();
        assert!(stopped.outcome.is_terminal());
        assert!(stopped.active_attempt().is_none());
        assert!(
            service
                .start_project(
                    p.id.clone(),
                    "after-shutdown".into(),
                    Authorization {
                        synthetic_data: true,
                        ..Default::default()
                    }
                )
                .await
                .is_err()
        );
        assert!(service.resume(expected_run_id.clone()).await.is_err());
        assert!(
            service
                .rerun(
                    expected_run_id,
                    request(&p, &provider, "rerun-after-shutdown")
                )
                .await
                .is_err()
        );
        service
            .events(Subscribe {
                run_id: original.id,
                after_sequence: 0,
            })
            .await
            .unwrap();
        assert_eq!(
            provider.request_count(),
            calls_at_return,
            "no invocation may occur after shutdown completes"
        );
    }
}
