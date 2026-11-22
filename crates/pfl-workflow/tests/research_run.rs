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
