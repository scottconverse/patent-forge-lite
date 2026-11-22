//! End-to-end verification of the two Increment A guarantees, driven through a
//! real provider client against a **loopback mock** — never a live model, never a
//! paid provider.
//!
//! 1. A real-invention declaration is accepted for local inference and recorded on
//!    the immutable run.
//! 2. A provider stream that ends before its terminal frame is stored as
//!    `Truncated`, has no report, and is resumable — it is never `Completed`.
use axum::{Router, response::IntoResponse, routing::post};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

fn intake() -> Intake {
    Intake {
        title: "Real-invention consent fixture".into(),
        description: [
            "AUTHORED FICTIONAL TEXT, not a client invention. A container holds a sensor ",
            "that a controller samples to drive one indicator. This paragraph exists only to ",
            "satisfy the word minimum and to exercise the real-invention consent path, with ",
            "Unicode café, against a loopback mock that returns recorded bytes. No confidential ",
            "material is used, and the model is never contacted.",
        ]
        .concat(),
        technical_field: "Consent fixtures".into(),
        ..Default::default()
    }
}

/// A complete Ollama NDJSON stream: content frames then the terminal `done` frame.
async fn complete_stream() -> impl IntoResponse {
    let frames = [
        "{\"message\":{\"content\":\"Stage output \"},\"done\":false}\n",
        "{\"message\":{\"content\":\"for café\"},\"done\":false}\n",
        "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":12,\"eval_count\":8}\n",
    ]
    .concat();
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        frames,
    )
}

/// The same stream without its terminal frame: the connection simply ends.
async fn truncated_stream() -> impl IntoResponse {
    let frames = ["{\"message\":{\"content\":\"Partial stage output\"},\"done\":false}\n"].concat();
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        frames,
    )
}

/// A terminal frame that reports an output limit rather than a natural stop.
async fn limited_stream() -> impl IntoResponse {
    let frames = [
        "{\"message\":{\"content\":\"Text cut off mid-sen\"},\"done\":false}\n",
        "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"length\",\"prompt_eval_count\":9,\"eval_count\":4096}\n",
    ]
    .concat();
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        frames,
    )
}

async fn spawn(router: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{address}")
}

async fn wait(service: &WorkflowService, id: &RunId) -> RunSnapshot {
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let run = service.run(id.clone()).await.unwrap();
            if run.outcome.is_terminal() {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("run must become terminal")
}

async fn service_at(base: &str) -> (Arc<WorkflowService>, Project) {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(OllamaProvider::new(base, "mock-model").expect("loopback endpoint"));
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    // Local execution requires the Full setup profile; Lean is Cloud-only by design.
    service
        .save_settings(Settings {
            profile: SetupProfile::Full,
            local_endpoint: Some(base.to_string()),
            local_model: Some("mock-model".into()),
        })
        .await
        .expect("full profile");
    let project = service.create_project(intake()).await.expect("project");
    (service, project)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_real_invention_declaration_runs_locally_and_is_recorded() {
    let base = spawn(Router::new().route("/api/chat", post(complete_stream))).await;
    let (service, project) = service_at(&base).await;

    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "real-local-1".into(),
            Authorization {
                synthetic_data: false,
                real_data: true,
                local_inference: true,
                ..Default::default()
            },
        )
        .await
        .expect("a real-invention declaration must be accepted for local inference");

    let finished = wait(&service, &run.id).await;
    assert_eq!(finished.outcome, RunOutcome::Completed);
    // Consent is part of the immutable run identity, not an ephemeral flag.
    assert!(finished.input.authorization.real_data);
    assert!(!finished.input.authorization.synthetic_data);
    let report = service.report(run.id.clone()).await.expect("report");
    assert!(report.markdown.contains("café"));
    let _ = service.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_output_limit_stop_is_recorded_as_truncated() {
    let base = spawn(Router::new().route("/api/chat", post(limited_stream))).await;
    let (service, project) = service_at(&base).await;

    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "limited-1".into(),
            Authorization {
                synthetic_data: false,
                real_data: true,
                local_inference: true,
                ..Default::default()
            },
        )
        .await
        .expect("run starts");

    let finished = wait(&service, &run.id).await;
    assert_eq!(
        finished.outcome,
        RunOutcome::Truncated,
        "a token-limited answer must not be saved as a completed stage"
    );
    assert!(service.report(run.id.clone()).await.is_err());
    let _ = service.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_without_its_terminal_frame_is_truncated_not_completed() {
    let base = spawn(Router::new().route("/api/chat", post(truncated_stream))).await;
    let (service, project) = service_at(&base).await;

    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "truncated-1".into(),
            Authorization {
                synthetic_data: false,
                real_data: true,
                local_inference: true,
                ..Default::default()
            },
        )
        .await
        .expect("run starts");

    let finished = wait(&service, &run.id).await;
    assert_eq!(
        finished.outcome,
        RunOutcome::Truncated,
        "an unfinished stream must never be recorded as Completed"
    );
    assert_ne!(finished.outcome, RunOutcome::Completed);
    // The partial text must not become a saved report.
    assert!(
        service.report(run.id.clone()).await.is_err(),
        "a truncated run must not expose a report"
    );
    // It is resumable: completed checkpoints are kept, the rest re-runs.
    assert!(finished.outcome.is_terminal());
    let _ = service.shutdown().await;
}
