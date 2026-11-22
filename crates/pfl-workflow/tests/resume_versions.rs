//! A resumed dependent workflow must re-run against the upstream versions it
//! recorded, not whatever the project has produced since.
//!
//! Compliance consumes a claim set. If claims are re-run after a compliance
//! attempt starts, resuming that attempt must still review the claims it was
//! created from — otherwise a saved review silently describes a different draft.
use async_trait::async_trait;
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::{Arc, Mutex};
use tokio::{runtime::Handle, sync::mpsc::UnboundedSender};

/// Wraps replay and records the prompt text each stage was asked to review.
struct RecordingProvider {
    inner: ReplayProvider,
    prompts: Arc<Mutex<Vec<String>>>,
    /// Makes each generated stage text unique, so two versions of the same
    /// workflow are distinguishable in the prompts later stages receive.
    counter: Mutex<u64>,
}

#[async_trait]
impl Provider for RecordingProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        self.inner.snapshot()
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: tokio_util::sync::CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        self.prompts
            .lock()
            .unwrap()
            .push(request.user_prompt.clone());
        let mut response = self.inner.generate(request, cancellation, deltas).await?;
        let mut counter = self.counter.lock().unwrap();
        *counter += 1;
        response.text.push_str(&format!("\n\nGEN-{}", *counter));
        Ok(response)
    }
}

fn intake() -> Intake {
    Intake {
        title: "Version ancestry fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. A fictional ",
            "container carries a temperature sensor sampled by a controller that drives one blue ",
            "indicator and reports an unavailable sensor instead of inventing a reading. A ",
            "removable fixture supplies deterministic synthetic readings for restart tests. This ",
            "paragraph exists only to satisfy the explicit word minimum and to exercise version ",
            "ancestry deterministically with recorded responses, Unicode café, and no real ",
            "invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "Synthetic fixtures".into(),
        ..Default::default()
    }
}

fn auth() -> Authorization {
    Authorization {
        synthetic_data: true,
        ..Default::default()
    }
}

async fn wait(service: &WorkflowService, id: &RunId) -> RunSnapshot {
    tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let run = service.run(id.clone()).await.unwrap();
            if run.outcome.is_terminal() {
                return run;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("terminal")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_compliance_reviews_the_claims_it_recorded() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let provider = Arc::new(RecordingProvider {
        inner: ReplayProvider::all_workflows(),
        prompts: prompts.clone(),
        counter: Mutex::new(0),
    });
    let service = WorkflowService::new(store, provider, Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    // Feasibility, then the claims set that compliance will review.
    let feasibility = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "f1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(
        wait(&service, &feasibility.id).await.outcome,
        RunOutcome::Completed
    );

    let claims_v1 = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "c1".into(),
            auth(),
        )
        .await
        .unwrap();
    let claims_v1 = wait(&service, &claims_v1.id).await;
    assert_eq!(claims_v1.outcome, RunOutcome::Completed);
    let recorded_claims = claims_v1
        .checkpoints
        .iter()
        .filter(|c| c.stage.kind() == WorkflowKind::Claims)
        .max_by_key(|c| c.stage.number())
        .map(|c| c.text.clone())
        .expect("claims text");

    // Start compliance against v1, then interrupt it so it can be resumed.
    let compliance = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Compliance,
            "comp1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(compliance.input.upstream.claims_text, recorded_claims);
    let _ = service.cancel(compliance.id.clone()).await;

    // A newer claims set now exists. It must not leak into the resumed review.
    let claims_v2 = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "c2".into(),
            auth(),
        )
        .await
        .unwrap();
    let claims_v2 = wait(&service, &claims_v2.id).await;
    assert_eq!(claims_v2.outcome, RunOutcome::Completed);
    let newer_claims = claims_v2
        .checkpoints
        .iter()
        .filter(|c| c.stage.kind() == WorkflowKind::Claims)
        .max_by_key(|c| c.stage.number())
        .map(|c| c.text.clone())
        .expect("claims text");

    assert_ne!(
        recorded_claims, newer_claims,
        "the two claims versions must be distinguishable for this test to mean anything"
    );

    prompts.lock().unwrap().clear();
    let resumed = service.resume(compliance.id.clone()).await.unwrap();
    assert_eq!(resumed.input.upstream.claims_text, recorded_claims);
    let finished = wait(&service, &resumed.id).await;
    assert_eq!(finished.outcome, RunOutcome::Completed);

    let seen = prompts.lock().unwrap().clone();
    assert!(!seen.is_empty(), "compliance stages must have run");
    for prompt in &seen {
        assert!(
            prompt.contains(recorded_claims.trim_end()),
            "resumed compliance must review the recorded claims version"
        );
        assert!(
            !prompt.contains(newer_claims.trim_end()),
            "resumed compliance must not pick up the newer claims version"
        );
    }
}
