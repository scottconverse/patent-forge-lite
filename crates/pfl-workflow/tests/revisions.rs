//! Edited document revisions: versioned, ordered, and anchored to the immutable
//! generation run they were edited from.
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::Arc;

fn intake() -> Intake {
    Intake {
        title: "Revision fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. A fictional ",
            "container carries a temperature sensor sampled by a controller that drives one blue ",
            "indicator and reports an unavailable sensor instead of inventing a reading. A ",
            "removable fixture supplies deterministic synthetic readings for restart tests. This ",
            "paragraph exists only to satisfy the explicit word minimum and to exercise revision ",
            "storage deterministically with recorded responses, Unicode café, and no real ",
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
async fn revisions_are_versioned_ordered_and_anchor_an_immutable_run() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    // A real generation run to edit from.
    let feasibility = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "rev-f1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(
        wait(&service, &feasibility.id).await.outcome,
        RunOutcome::Completed
    );
    let claims_run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "rev-c1".into(),
            auth(),
        )
        .await
        .unwrap();
    let claims_run = wait(&service, &claims_run.id).await;
    assert_eq!(claims_run.outcome, RunOutcome::Completed);
    let original_claims = claims_run
        .checkpoints
        .iter()
        .filter(|c| c.stage.kind() == WorkflowKind::Claims)
        .max_by_key(|c| c.stage.number())
        .map(|c| c.text.clone())
        .expect("claims text");

    let edited = format!("{original_claims}\n\n1. (Independent) An edited claim.");
    let first = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Claims,
            Some(claims_run.id.clone()),
            edited.clone(),
            "manual edit".into(),
        )
        .await
        .expect("first revision");
    assert_eq!(first.version, 1);
    assert_eq!(first.base_run_id.as_ref(), Some(&claims_run.id));

    let second = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Claims,
            Some(claims_run.id.clone()),
            format!("{edited}\n\n2. (Dependent on 1) A further edited claim."),
            "second edit".into(),
        )
        .await
        .expect("second revision");
    assert_eq!(second.version, 2);

    // Ordered, complete, and retrievable by id.
    let all = service
        .revisions(project.id.clone(), DocumentKind::Claims)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].version, 1);
    assert_eq!(all[1].version, 2);
    assert_eq!(all[1].text, second.text);
    let fetched = service.revision(first.id.clone()).await.unwrap();
    assert_eq!(fetched.text, edited);

    // The generation run is untouched by the edits.
    let reloaded = service.run(claims_run.id.clone()).await.unwrap();
    let reloaded_text = reloaded
        .checkpoints
        .iter()
        .filter(|c| c.stage.kind() == WorkflowKind::Claims)
        .max_by_key(|c| c.stage.number())
        .map(|c| c.text.clone())
        .unwrap();
    assert_eq!(reloaded_text, original_claims, "edits never mutate the run");
    assert_eq!(reloaded.version, claims_run.version);

    // Application revisions are a separate series.
    let application = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Application,
            None,
            "Draft specification text.".into(),
            "scratch".into(),
        )
        .await
        .unwrap();
    assert_eq!(application.version, 1, "versioning is per document kind");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compliance_reviews_the_selected_revisions_not_the_latest_runs() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    let feasibility = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "sel-f".into(),
            auth(),
        )
        .await
        .unwrap();
    wait(&service, &feasibility.id).await;
    let claims_run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "sel-c".into(),
            auth(),
        )
        .await
        .unwrap();
    let claims_run = wait(&service, &claims_run.id).await;

    let claims_revision = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Claims,
            Some(claims_run.id.clone()),
            "SELECTED CLAIMS REVISION TEXT".into(),
            "edited".into(),
        )
        .await
        .unwrap();
    let spec_revision = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Application,
            None,
            "SELECTED SPECIFICATION REVISION TEXT".into(),
            "draft".into(),
        )
        .await
        .unwrap();

    let compliance = service
        .start_workflow_with_selection(
            project.id.clone(),
            WorkflowKind::Compliance,
            "sel-k".into(),
            auth(),
            Vec::new(),
            Some(claims_revision.id.clone()),
            Some(spec_revision.id.clone()),
        )
        .await
        .expect("compliance against selected revisions");

    assert_eq!(
        compliance.input.upstream.claims_text,
        "SELECTED CLAIMS REVISION TEXT"
    );
    assert_eq!(
        compliance.input.upstream.specification_text,
        "SELECTED SPECIFICATION REVISION TEXT"
    );
    assert_eq!(
        compliance.input.upstream.claims_revision_id.as_ref(),
        Some(&claims_revision.id)
    );
    assert_eq!(
        compliance.input.upstream.specification_revision_id.as_ref(),
        Some(&spec_revision.id)
    );
    assert_eq!(
        wait(&service, &compliance.id).await.outcome,
        RunOutcome::Completed
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_revision_cannot_anchor_a_run_of_another_document_or_project() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();
    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "mismatch".into(),
            auth(),
        )
        .await
        .unwrap();

    let error = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Claims,
            Some(run.id.clone()),
            "text".into(),
            String::new(),
        )
        .await
        .expect_err("a claims revision cannot anchor a feasibility run");
    assert!(error.to_string().contains("same project and document"));
}

/// B5's core invariant: a regeneration makes a new version and leaves the base run
/// and its revisions exactly as they were.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rerun_creates_a_new_version_and_leaves_the_base_untouched() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    let base = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "b5-base".into(),
            auth(),
        )
        .await
        .unwrap();
    let base = wait(&service, &base.id).await;
    assert_eq!(base.outcome, RunOutcome::Completed);
    let base_checkpoints = base.checkpoints.clone();
    let base_report = base.report.clone();

    let revision = service
        .save_revision(
            project.id.clone(),
            DocumentKind::Claims,
            None,
            "1. An edited claim.".into(),
            "kept across regeneration".into(),
        )
        .await
        .unwrap();

    let regenerated = service
        .rerun(
            base.id.clone(),
            NewRun {
                id: RunId::new(),
                project_id: project.id.clone(),
                idempotency_key: "b5-rerun".into(),
                input: base.input.clone(),
                rerun_of: None,
                created_at_ms: now_ms(),
            },
        )
        .await
        .expect("a rerun of a completed run is allowed");
    assert_ne!(regenerated.id, base.id, "a rerun gets a new identity");
    assert_eq!(
        regenerated.rerun_of.as_ref(),
        Some(&base.id),
        "lineage is recorded"
    );
    assert_eq!(
        regenerated.version,
        base.version + 1,
        "a rerun is the next version"
    );
    let regenerated = wait(&service, &regenerated.id).await;
    assert_eq!(regenerated.outcome, RunOutcome::Completed);

    // The base run is untouched: same checkpoints, same report, same version.
    let reread = service.run(base.id.clone()).await.unwrap();
    assert_eq!(reread.checkpoints, base_checkpoints);
    assert_eq!(reread.report, base_report);
    assert_eq!(reread.version, base.version);

    // The revision survives, still anchored to whatever it was anchored to.
    let revisions = service
        .revisions(project.id.clone(), DocumentKind::Claims)
        .await
        .unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].id, revision.id);
    assert_eq!(revisions[0].text, "1. An edited claim.");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_committed_stages_are_offered_for_regeneration() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "regen".into(),
            auth(),
        )
        .await
        .unwrap();
    // A running run offers nothing: a selection cannot name an unproduced stage.
    let error = service
        .regenerable_stages(run.id.clone())
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("terminal"),
        "unexpected: {error}"
    );

    let done = wait(&service, &run.id).await;
    assert_eq!(done.outcome, RunOutcome::Completed);
    let stages = service.regenerable_stages(done.id.clone()).await.unwrap();
    assert_eq!(
        stages,
        done.checkpoints.iter().map(|c| c.stage).collect::<Vec<_>>()
    );
    assert!(
        !stages.is_empty(),
        "a completed run offers its committed stages"
    );
}
