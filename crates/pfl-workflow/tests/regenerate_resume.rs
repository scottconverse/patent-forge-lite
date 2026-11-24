//! A regenerated run that is interrupted must resume, and its carried prefix must stay
//! byte-identical through that resume.
//!
//! Before this test, `drive` rebuilt and re-committed the carried checkpoints on every
//! attempt with the new attempt id, which the store refuses because a completed stage is
//! immutable. An interrupted regeneration could therefore never be recovered, although the
//! documentation promised that resume works normally.
use async_trait::async_trait;
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{runtime::Handle, sync::mpsc::UnboundedSender};

/// Fails the next call once, then behaves like replay. One failure is enough to leave a
/// regenerated run interrupted with its prefix already committed.
struct FailOnceProvider {
    inner: ReplayProvider,
    fail_next: AtomicBool,
}

#[async_trait]
impl Provider for FailOnceProvider {
    fn snapshot(&self) -> ProviderSnapshot {
        self.inner.snapshot()
    }
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: tokio_util::sync::CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(Error::Provider("synthetic interruption".into()));
        }
        self.inner.generate(request, cancellation, deltas).await
    }
}

fn intake() -> Intake {
    Intake {
        title: "Regenerated resume fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature ",
            "sensor sampled by a controller that reports an unavailable sensor instead of ",
            "inventing a reading. This paragraph exists only to satisfy the explicit word ",
            "minimum and to exercise resuming an interrupted regeneration deterministically, ",
            "with café Unicode and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

fn auth() -> Authorization {
    Authorization {
        synthetic_data: true,
        ..Default::default()
    }
}

async fn wait(service: &Arc<WorkflowService>, id: &RunId) -> RunSnapshot {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let run = service.run(id.clone()).await.unwrap();
        if run.outcome.is_terminal() || std::time::Instant::now() > deadline {
            return run;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_interrupted_regeneration_resumes_with_its_carried_prefix_intact() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(FailOnceProvider {
        inner: ReplayProvider::all_workflows(),
        fail_next: AtomicBool::new(false),
    });
    let service = WorkflowService::new(store, provider.clone(), Handle::current());

    let project = service.create_project(intake()).await.unwrap();
    let base = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "base".into(),
            auth(),
        )
        .await
        .unwrap();
    let base = wait(&service, &base.id).await;
    assert_eq!(base.outcome, RunOutcome::Completed);
    let stages = base.input.kind.stages();
    let restart_index = 3usize;
    let restart = stages[restart_index];

    // The regeneration stops at its first new stage, leaving the carried prefix committed.
    provider.fail_next.store(true, Ordering::SeqCst);
    let regenerated = service
        .regenerate_from(base.id.clone(), restart, auth())
        .await
        .unwrap();
    let interrupted = wait(&service, &regenerated.id).await;
    assert_eq!(interrupted.outcome, RunOutcome::Failed);
    assert_eq!(
        interrupted.checkpoints.len(),
        restart_index,
        "the carried prefix should be committed before the failure"
    );

    // Resuming must re-run the remaining stages without re-committing the prefix.
    let resumed = service
        .resume(interrupted.id.clone(), auth())
        .await
        .unwrap();
    let finished = wait(&service, &resumed.id).await;
    assert_eq!(finished.outcome, RunOutcome::Completed);
    assert_eq!(finished.checkpoints.len(), stages.len());
    for (carried, original) in finished
        .checkpoints
        .iter()
        .zip(base.checkpoints.iter())
        .take(restart_index)
    {
        assert_eq!(carried.stage, original.stage);
        assert_eq!(
            carried.text, original.text,
            "the carried stage must be byte-identical after a resume"
        );
    }
    assert_eq!(
        finished
            .checkpoints
            .iter()
            .map(|checkpoint| checkpoint.stage)
            .collect::<Vec<_>>()
            .len(),
        stages.len(),
        "no stage may be duplicated by the resume"
    );
}
