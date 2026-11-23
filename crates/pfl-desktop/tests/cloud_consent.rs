//! Cloud consent driven through the session actor, headlessly.
//!
//! This is A5's substance without the window: a declined paid run must be refused before
//! any provider call, and an approved one must reach the provider and complete. The
//! provider is a loopback SSE stand-in selected through `PFL_CLOUD_ENDPOINT`, so no live
//! service is contacted and nothing is spent.
//!
//! Written as a plain `#[test]`: the session actor owns a private runtime, and wrapping
//! this in `#[tokio::test]` starves it while the test thread blocks waiting for replies.
use axum::{Router, http::StatusCode, response::IntoResponse, routing::post};
use pfl_desktop::session::{ApplicationSession, Mode, Reply, Request};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::ReplayProvider;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const STAGES: usize = 6;

fn intake() -> Intake {
    Intake {
        title: "Cloud consent fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature ",
            "sensor sampled by a controller that reports an unavailable sensor instead of ",
            "inventing a reading. This paragraph exists only to satisfy the word minimum ",
            "and to exercise the paid Cloud path against a loopback stand-in, with café ",
            "Unicode and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

/// Serves one valid SSE turn per request on its own runtime thread.
fn spawn_cloud(calls: Arc<AtomicUsize>) -> String {
    let (ready, address) = mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            ready
                .send(listener.local_addr().unwrap())
                .expect("address channel");
            let handler = post(move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    let body = concat!(
                        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n",
                        "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Loopback stage output.\"}}\n\n",
                        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":11}}\n\n",
                        "data: {\"type\":\"message_stop\"}\n\n"
                    );
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                        body,
                    )
                        .into_response()
                }
            });
            axum::serve(listener, Router::new().route("/v1/messages", handler))
                .await
                .unwrap();
        });
    });
    let address = address.recv().expect("mock address");
    // The provider appends /v1/messages, so this is the base URL only.
    format!("http://{address}")
}

/// Every snapshot seen while waiting, so a run that finishes before its id is known is not
/// lost. A fast platform can deliver the terminal Poll before the Start reply is read.
#[derive(Debug, Default)]
struct Seen {
    snapshots: Vec<RunSnapshot>,
    labels: Vec<String>,
}

fn wait_for(
    session: &ApplicationSession,
    label: &str,
    seen: &mut Seen,
    mut ready: impl FnMut(&Reply) -> bool,
) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(120);
    while Instant::now() < deadline {
        for reply in session.drain() {
            if let Reply::Poll { events, snapshot } = &reply {
                seen.snapshots.push(snapshot.clone());
                for event in events {
                    seen.labels.push(format!("{event:?}"));
                }
            }
            if ready(&reply) {
                return reply;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no matching reply within the deadline at {label}; seen: {seen:?}");
}

#[test]
fn a_declined_cloud_run_sends_nothing_and_an_approved_one_completes() {
    let calls = Arc::new(AtomicUsize::new(0));
    let endpoint = spawn_cloud(calls.clone());
    // SAFETY: this test binary runs a single test, and the variable is set before the
    // session is built and cleared before it returns.
    unsafe { std::env::set_var("PFL_CLOUD_ENDPOINT", &endpoint) };

    let mut seen = Seen::default();
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let session =
        ApplicationSession::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
            .unwrap();

    session
        .request(Request::ConfigureCloud {
            model: "claude-mock".into(),
            key: "sk-ant-headless".into(),
        })
        .unwrap();
    match wait_for(&session, "configure", &mut seen, |reply| {
        matches!(reply, Reply::Configured(_) | Reply::Error(_))
    }) {
        Reply::Configured(Mode::Cloud { .. }) => {}
        other => panic!("unexpected configuration reply {other:?}"),
    }

    session.request(Request::Create(intake())).unwrap();
    let project = match wait_for(&session, "create", &mut seen, |reply| {
        matches!(reply, Reply::Opened(_, _) | Reply::Error(_))
    }) {
        Reply::Opened(project, _) => project,
        other => panic!("unexpected create reply {other:?}"),
    };

    // Declined: no paid authorization, so the run is refused before any provider call.
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "cloud-declined".into(),
            authorization: Authorization {
                synthetic_data: true,
                paid_provider: false,
                ..Default::default()
            },
        })
        .unwrap();
    match wait_for(&session, "declined start", &mut seen, |reply| {
        matches!(reply, Reply::Error(_) | Reply::Run(_))
    }) {
        Reply::Error(error) => assert!(
            error.to_lowercase().contains("paid") || error.to_lowercase().contains("cloud"),
            "the refusal should name the paid approval, got: {error}"
        ),
        other => panic!("a declined Cloud run must be refused, got {other:?}"),
    }
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a declined run must not contact the provider"
    );

    // Approved: the same run reaches the stand-in and completes every stage.
    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "cloud-approved".into(),
            authorization: Authorization {
                synthetic_data: true,
                paid_provider: true,
                ..Default::default()
            },
        })
        .unwrap();
    // Wait for the run this Start created, by id: an earlier declined attempt leaves its own
    // run selected, and matching on "some terminal run" would read the wrong one.
    let started = match wait_for(&session, "approved start", &mut seen, |reply| {
        matches!(reply, Reply::Run(_) | Reply::Error(_))
    }) {
        Reply::Run(run) => run,
        other => panic!("the approved start did not produce a run: {other:?}"),
    };
    let approved_id = started.id.clone();
    // Ask the session for the run by id instead of relying on poll timing: a fast
    // platform can finish the run before any Poll reply is emitted.
    let mut run = started.clone();
    let deadline = Instant::now() + Duration::from_secs(120);
    while !run.outcome.is_terminal() && Instant::now() < deadline {
        session
            .request(Request::SelectRun(approved_id.clone()))
            .unwrap();
        match wait_for(&session, "select run", &mut seen, |reply| {
            matches!(reply, Reply::Run(_) | Reply::Error(_))
        }) {
            Reply::Run(snapshot) => run = snapshot,
            other => panic!("selecting the run failed: {other:?}"),
        }
        if !run.outcome.is_terminal() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    // A terminal snapshot may already have been seen before the id was known.

    assert_eq!(run.outcome, RunOutcome::Completed);
    assert_eq!(run.checkpoints.len(), STAGES);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        STAGES,
        "one request per stage, no retries and no fallback"
    );

    unsafe { std::env::remove_var("PFL_CLOUD_ENDPOINT") };
}
