//! PatentsView wired into a run: opt-in research records verified references, and a
//! run without opt-in queries nothing.
use axum::{Router, http::StatusCode, response::IntoResponse, routing::post};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::net::TcpListener;

fn intake() -> Intake {
    Intake {
        title: "PatentsView fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature sensor ",
            "sampled by a controller that drives one blue indicator and reports an unavailable ",
            "sensor instead of inventing a reading. This paragraph exists only to satisfy the ",
            "word minimum and to exercise opt-in research with recorded responses and Unicode ",
            "café, and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

async fn spawn_patentsview() -> (String, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = calls.clone();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/api/v1/patent/",
                post(move || {
                    let counter = counter.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        (
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            r#"{"patents":[{"patent_id":"10123456","patent_title":"Container with temperature indicator","patent_date":"2022-05-17","assignee_organization":"Acme Sensors"}]}"#,
                        )
                            .into_response()
                    }
                }),
            ),
        )
        .await
        .unwrap();
    });
    (format!("http://{address}/api/v1/patent/"), calls)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn opt_in_research_records_verified_references_and_opt_out_queries_nothing() {
    let (endpoint, calls) = spawn_patentsview().await;
    let source = Arc::new(PatentsViewSource::with_endpoint("key", &endpoint).unwrap());
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider: Arc<dyn Provider> = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new_with_research(
        store,
        provider,
        tokio::runtime::Handle::current(),
        Some(source),
    );
    let project = service.create_project(intake()).await.unwrap();

    // Opted in: the query runs and its records land in the immutable run input.
    let opted_in = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "research-on".into(),
            Authorization {
                synthetic_data: true,
                research: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let done = wait(&service, &opted_in.id).await;
    assert_eq!(done.outcome, RunOutcome::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "exactly one query per run");
    let prior_art = &done.input.upstream.prior_art;
    assert_eq!(prior_art.len(), 1);
    assert_eq!(prior_art[0].publication_number, "10123456");
    assert!(prior_art[0].verified);

    // A retry with the same idempotency key replays the stored run rather than disclosing
    // the same title to PatentsView a second time.
    let replay = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "research-on".into(),
            Authorization {
                synthetic_data: true,
                research: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(replay.id, done.id, "the same key returns the same run");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "a repeated key must not query again"
    );

    // Opted out: no query, no references, and nothing silently invented.
    let opted_out = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "research-off".into(),
            Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let done = wait(&service, &opted_out.id).await;
    assert_eq!(done.outcome, RunOutcome::Completed);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "no query may happen without opt-in"
    );
    assert!(done.input.upstream.prior_art.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_research_query_fails_the_run_instead_of_recording_nothing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/api/v1/patent/",
                post(|| async { (StatusCode::FORBIDDEN, "denied") }),
            ),
        )
        .await
        .unwrap();
    });
    let endpoint = format!("http://{address}/api/v1/patent/");
    let source = Arc::new(PatentsViewSource::with_endpoint("key", &endpoint).unwrap());
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider: Arc<dyn Provider> = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new_with_research(
        store,
        provider,
        tokio::runtime::Handle::current(),
        Some(source),
    );
    let project = service.create_project(intake()).await.unwrap();

    let run = service
        .start_workflow(
            project.id,
            WorkflowKind::Feasibility,
            "research-denied".into(),
            Authorization {
                synthetic_data: true,
                research: true,
                ..Default::default()
            },
        )
        .await;
    // Either the start is refused or the run fails; what must not happen is a
    // completed run with an empty reference set.
    match run {
        Err(error) => assert!(error.to_string().contains("PatentsView")),
        Ok(snapshot) => {
            let done = wait(&service, &snapshot.id).await;
            assert_eq!(done.outcome, RunOutcome::Failed);
            assert!(done.input.upstream.prior_art.is_empty());
        }
    }
}

async fn wait(service: &WorkflowService, id: &RunId) -> RunSnapshot {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let run = service.run(id.clone()).await.unwrap();
            if run.outcome.is_terminal() {
                return run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("terminal")
}

/// A refused start must not send the invention to the prior-art service: consent is decided
/// before anything leaves the machine.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refused_start_makes_no_research_call() {
    let (endpoint, calls) = spawn_patentsview().await;
    let source = Arc::new(PatentsViewSource::with_endpoint("key", &endpoint).unwrap());
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider: Arc<dyn Provider> = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new_with_research(
        store,
        provider,
        tokio::runtime::Handle::current(),
        Some(source),
    );
    let project = service.create_project(intake()).await.unwrap();

    // Real-invention data may not reach PatentsView, so the start is refused.
    let refused = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "research-real".into(),
            Authorization {
                real_data: true,
                local_inference: true,
                research: true,
                ..Default::default()
            },
        )
        .await;
    assert!(refused.is_err(), "real data with research must be refused");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "a refused start must not query the prior-art service"
    );
}

/// A response without a `patents` array, or one flagged as an error, is a failed search —
/// never "no prior art exists".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_error_shaped_response_is_a_failure_not_an_empty_result() {
    async fn serve(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/api/v1/patent/",
                    post(move || async move {
                        (
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            body,
                        )
                            .into_response()
                    }),
                ),
            )
            .await
            .unwrap();
        });
        format!("http://{address}/api/v1/patent/")
    }

    for (label, body) in [
        ("no array", r#"{"message":"ok"}"#),
        ("error flag", r#"{"error":true}"#),
    ] {
        let endpoint = serve(body).await;
        let source = Arc::new(PatentsViewSource::with_endpoint("key", &endpoint).unwrap());
        let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
        let provider: Arc<dyn Provider> = Arc::new(ReplayProvider::all_workflows());
        let service = WorkflowService::new_with_research(
            store,
            provider,
            tokio::runtime::Handle::current(),
            Some(source),
        );
        let project = service.create_project(intake()).await.unwrap();
        let refused = service
            .start_workflow(
                project.id.clone(),
                WorkflowKind::Feasibility,
                format!("research-{label}"),
                Authorization {
                    synthetic_data: true,
                    research: true,
                    ..Default::default()
                },
            )
            .await;
        assert!(
            refused.is_err(),
            "{label}: an error-shaped body must fail the start, not record an empty set"
        );
    }
}
