//! The PatentsView key and opt-in research, through the session actor, headlessly.
//!
//! Same shape as the Cloud consent test: configure research with a key against a loopback
//! stand-in, run with the opt-in, confirm the reference is recorded and exactly one query
//! was made, and then prove the key never reached the database — with the scan itself
//! proven against a known persisted value.
use axum::{Router, http::StatusCode, response::IntoResponse, routing::post};
use pfl_desktop::session::{ApplicationSession, Mode, Reply, Request};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::ReplayProvider;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};

fn intake() -> Intake {
    Intake {
        title: "Research consent fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature ",
            "sensor sampled by a controller that reports an unavailable sensor instead of ",
            "inventing a reading. This paragraph exists only to satisfy the word minimum ",
            "and to exercise opt-in prior-art retrieval against a loopback stand-in, with ",
            "café Unicode and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

fn spawn_patentsview(calls: Arc<AtomicUsize>) -> String {
    let (ready, address) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            ready.send(listener.local_addr().unwrap()).unwrap();
            let handler = post(move || {
                let calls = calls.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        r#"{"patents":[{"patent_id":"10123456","patent_title":"Container with temperature indicator","patent_date":"2022-05-17","assignee_organization":"Acme Sensors"}]}"#,
                    )
                        .into_response()
                }
            });
            axum::serve(listener, Router::new().route("/api/v1/patent/", handler))
                .await
                .unwrap();
        });
    });
    let address = address.recv().expect("mock address");
    format!("http://{address}/api/v1/patent/")
}

fn drain_until(
    session: &ApplicationSession,
    ready: impl FnMut(&Reply) -> bool,
    label: &str,
) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut ready = ready;
    while Instant::now() < deadline {
        for reply in session.drain() {
            if ready(&reply) {
                return reply;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no matching reply within the deadline at {label}");
}

#[test]
fn the_research_key_never_reaches_the_database_and_opt_in_makes_one_query() {
    let calls = Arc::new(AtomicUsize::new(0));
    let endpoint = spawn_patentsview(calls.clone());
    // SAFETY: this test binary runs a single test; the variable is set before the session
    // reads it and cleared before returning.
    unsafe { std::env::set_var("PFL_RESEARCH_ENDPOINT", &endpoint) };

    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("projects.sqlite3");
    let store: Arc<dyn Store> = Arc::new(SqliteStore::open(&database).unwrap());
    let session =
        ApplicationSession::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
            .unwrap();

    // A run needs an execution mode; research configuration preserves whatever is set, so
    // configure replay first and then attach the research source.
    session.request(Request::Configure(Mode::Replay)).unwrap();
    match drain_until(
        &session,
        |reply| matches!(reply, Reply::Configured(_) | Reply::Error(_)),
        "configure replay",
    ) {
        Reply::Configured(_) => {}
        other => panic!("configuring replay failed: {other:?}"),
    }

    session
        .request(Request::ConfigureResearch {
            api_key: "patentsview-secret-test-key".into(),
        })
        .unwrap();
    match drain_until(
        &session,
        |reply| matches!(reply, Reply::ResearchConfigured | Reply::Error(_)),
        "configure research",
    ) {
        Reply::ResearchConfigured => {}
        other => panic!("research configuration failed: {other:?}"),
    }

    session.request(Request::Create(intake())).unwrap();
    let project = match drain_until(
        &session,
        |reply| matches!(reply, Reply::Opened(_, _) | Reply::Error(_)),
        "create",
    ) {
        Reply::Opened(project, _) => project,
        other => panic!("project creation failed: {other:?}"),
    };

    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "research-opt-in".into(),
            authorization: Authorization {
                synthetic_data: true,
                research: true,
                ..Default::default()
            },
        })
        .unwrap();
    let started = match drain_until(
        &session,
        |reply| matches!(reply, Reply::Run(_) | Reply::Error(_)),
        "start",
    ) {
        Reply::Run(run) => run,
        other => panic!("the opted-in run did not start: {other:?}"),
    };

    // Ask for the run by id rather than waiting on poll timing.
    let run_id = started.id.clone();
    let mut run = started;
    let deadline = Instant::now() + Duration::from_secs(60);
    while !run.outcome.is_terminal() && Instant::now() < deadline {
        session.request(Request::SelectRun(run_id.clone())).unwrap();
        match drain_until(
            &session,
            |reply| matches!(reply, Reply::Run(_) | Reply::Error(_)),
            "select run",
        ) {
            Reply::Run(snapshot) => run = snapshot,
            other => panic!("selecting the run failed: {other:?}"),
        }
        if !run.outcome.is_terminal() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert_eq!(run.outcome, RunOutcome::Completed);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "an opted-in run makes exactly one query"
    );
    assert_eq!(run.input.upstream.prior_art.len(), 1);
    assert_eq!(
        run.input.upstream.prior_art[0].publication_number,
        "10123456"
    );

    drop(session);
    let bytes = std::fs::read(&database).unwrap();
    // Prove the scan finds persisted text before trusting its silence about the key.
    let title = b"Research consent fixture";
    assert!(
        bytes.windows(title.len()).any(|window| window == title),
        "the scan should find the project title in the database"
    );
    let needle = b"patentsview-secret-test-key";
    assert!(
        !bytes.windows(needle.len()).any(|window| window == needle),
        "the PatentsView key must never reach the database"
    );

    unsafe { std::env::remove_var("PFL_RESEARCH_ENDPOINT") };
}
