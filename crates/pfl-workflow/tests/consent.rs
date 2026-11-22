//! Recorded consent, endpoint policy and snapshot compatibility.
//!
//! These tests never contact a provider: the consent decision is made before any
//! network call, and old snapshots must still deserialize after the schema gain.
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::Arc;
use tokio::runtime::Handle;

fn intake() -> Intake {
    Intake {
        title: "Consent fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
            "A fictional container carries a sensor sampled by a controller that drives ",
            "one indicator and reports an unavailable sensor instead of inventing a ",
            "reading. This text exists only to satisfy the explicit word minimum and to ",
            "exercise the consent rules deterministically with recorded responses, ",
            "Unicode café, and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "Synthetic fixtures".into(),
        ..Default::default()
    }
}

fn service() -> Arc<WorkflowService> {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    WorkflowService::new(
        store,
        Arc::new(ReplayProvider::all_workflows()),
        Handle::current(),
    )
}

async fn project(service: &Arc<WorkflowService>) -> ProjectId {
    service.create_project(intake()).await.expect("project").id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_without_a_data_declaration_is_refused() {
    let service = service();
    let id = project(&service).await;
    let error = service
        .start_workflow(
            id,
            WorkflowKind::Feasibility,
            "undeclared".into(),
            Authorization::default(),
        )
        .await
        .expect_err("no declaration must be refused");
    assert!(
        error.to_string().contains("declare the run's data first"),
        "unexpected error: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recorded_replay_cannot_claim_a_real_invention() {
    let service = service();
    let id = project(&service).await;
    let error = service
        .start_workflow(
            id,
            WorkflowKind::Feasibility,
            "real-on-replay".into(),
            Authorization {
                real_data: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("replay is synthetic output and must be refused for real data");
    assert!(
        error
            .to_string()
            .contains("recorded replay produces synthetic output"),
        "unexpected error: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn synthetic_replay_records_its_declaration() {
    let service = service();
    let id = project(&service).await;
    // Replay is synthetic by construction, so the positive case it can assert is
    // that the declaration it requires is recorded faithfully. Real-invention
    // acceptance is exercised against the local adapter in run_outcomes.rs.
    let run = service
        .start_workflow(
            id,
            WorkflowKind::Feasibility,
            "synthetic-ok".into(),
            Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        )
        .await
        .expect("synthetic replay run");
    assert!(run.input.authorization.synthetic_data);
    assert!(!run.input.authorization.real_data);
    let _ = service.cancel(run.id).await;
    let _ = service.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_run_cannot_claim_both_declarations() {
    let service = service();
    let id = project(&service).await;
    let error = service
        .start_workflow(
            id,
            WorkflowKind::Feasibility,
            "both-declarations".into(),
            Authorization {
                synthetic_data: true,
                real_data: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("exactly one declaration is allowed");
    assert!(
        error.to_string().contains("exactly one data declaration"),
        "unexpected error: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_invention_data_cannot_be_sent_to_cloud() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    // A cloud adapter with a placeholder endpoint: the policy refuses before any call.
    let provider =
        Arc::new(CloudProvider::with_endpoint("key", "model", "http://127.0.0.1:1").unwrap());
    let service = WorkflowService::new(store, provider, Handle::current());
    let id = project(&service).await;
    let error = service
        .start_workflow(
            id,
            WorkflowKind::Feasibility,
            "real-cloud".into(),
            Authorization {
                real_data: true,
                paid_provider: true,
                ..Default::default()
            },
        )
        .await
        .expect_err("real data is local-only in this build");
    assert!(
        error.to_string().contains("local-only"),
        "unexpected error: {error}"
    );
}

#[test]
fn old_snapshots_without_a_data_declaration_still_deserialize() {
    // A run recorded before the field existed must load, with the new field
    // defaulting to false rather than failing to parse.
    let old =
        r#"{"synthetic_data":true,"local_inference":true,"paid_provider":false,"research":false}"#;
    let authorization: Authorization = serde_json::from_str(old).expect("old snapshot parses");
    assert!(authorization.synthetic_data);
    assert!(!authorization.real_data, "absent field defaults to false");
    assert!(authorization.local_inference);

    let real = r#"{"synthetic_data":false,"real_data":true,"local_inference":true,"paid_provider":false,"research":false}"#;
    let authorization: Authorization = serde_json::from_str(real).expect("new snapshot parses");
    assert!(authorization.real_data);
    assert!(!authorization.synthetic_data);
}

#[test]
fn remote_runtimes_cannot_be_constructed_as_on_machine_local_inference() {
    // The guard lives at construction, so no call can be attempted first.
    assert!(OllamaProvider::new("http://192.0.2.10:11434", "phi4-mini:latest").is_err());
    assert!(OllamaProvider::new("https://ollama.example.com", "phi4-mini:latest").is_err());
    assert!(OllamaProvider::new("http://127.0.0.1:11434", "phi4-mini:latest").is_ok());
    assert!(OllamaProvider::new("http://localhost:11434", "phi4-mini:latest").is_ok());
    assert!(OllamaProvider::new("http://[::1]:11434", "phi4-mini:latest").is_ok());
}
