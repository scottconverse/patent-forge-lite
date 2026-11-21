//! End-to-end coverage of the three dependent workflows, on top of the
//! feasibility slice that already has its own suite.
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::{sync::Arc, time::Duration};
use tokio::runtime::Handle;

fn intake() -> Intake {
    Intake {
        title: "Synthetic multi-workflow fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
            "A fictional container carries a sensor sampled by a controller that drives ",
            "one indicator and reports an unavailable sensor instead of inventing a ",
            "reading. This text exists only to satisfy the explicit word minimum and to ",
            "exercise every workflow stage deterministically with recorded responses, ",
            "Unicode café, and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "Synthetic fixtures".into(),
        ..Default::default()
    }
}

fn authorization() -> Authorization {
    Authorization {
        synthetic_data: true,
        ..Default::default()
    }
}

async fn wait(service: &WorkflowService, id: &RunId) -> RunSnapshot {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let run = service.run(id.clone()).await.unwrap();
            if run.outcome.is_terminal() {
                return run;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("run must finish")
}

/// Runs feasibility, then returns the service so dependants can be exercised.
async fn with_feasibility() -> (
    Arc<WorkflowService>,
    Arc<ReplayProvider>,
    Project,
    RunSnapshot,
) {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider.clone(), Handle::current());
    let project = service.create_project(intake()).await.unwrap();
    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "feasibility-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let completed = wait(&service, &run.id).await;
    assert_eq!(completed.outcome, RunOutcome::Completed);
    (service, provider, project, completed)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dependent_workflows_refuse_to_start_without_their_prerequisites() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider.clone(), Handle::current());
    let project = service.create_project(intake()).await.unwrap();
    for kind in [
        WorkflowKind::Claims,
        WorkflowKind::Compliance,
        WorkflowKind::Application,
    ] {
        let error = service
            .start_workflow(
                project.id.clone(),
                kind,
                kind.slug().to_string(),
                authorization(),
            )
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("must complete"),
            "{} should be refused before its prerequisites, got {error}",
            kind.slug()
        );
    }
    // No refused attempt may leave a run, nor a provider call.
    assert!(service.history(project.id).await.unwrap().is_empty());
    assert_eq!(provider.request_count(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn claims_run_consumes_snapshotted_feasibility_and_reports_the_revision() {
    let (service, provider, project, feasibility) = with_feasibility().await;
    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "claims-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let claims = wait(&service, &run.id).await;
    assert_eq!(claims.outcome, RunOutcome::Completed);
    assert_eq!(claims.input.kind, WorkflowKind::Claims);
    assert_eq!(
        claims
            .checkpoints
            .iter()
            .map(|c| c.stage)
            .collect::<Vec<_>>(),
        Stage::CLAIMS
    );
    // The upstream text is snapshotted, not re-read later.
    let stage_five = feasibility
        .checkpoints
        .iter()
        .find(|c| c.stage == Stage::IpStrategy)
        .unwrap()
        .text
        .clone();
    assert_eq!(claims.input.upstream.stage(Stage::IpStrategy), stage_five);
    assert!(!claims.input.upstream.stage(Stage::FinalReport).is_empty());
    // Both upstream stages reached the planner request unchanged.
    let planner = provider
        .requests()
        .into_iter()
        .find(|r| r.stage == Stage::ClaimsPlan)
        .unwrap();
    assert!(planner.user_prompt.contains(&stage_five));
    assert!(
        planner
            .user_prompt
            .contains("IP Strategy & Recommendations (Stage 5)")
    );
    // The report is the revision pass, and the prompt version is claims-specific.
    let report = claims.report.clone().unwrap().markdown;
    assert!(report.contains("Claim revision"));
    assert_eq!(
        claims.input.prompts,
        prompts::snapshot(WorkflowKind::Claims)
    );
    assert_ne!(
        claims.input.prompts.version,
        prompts::snapshot(WorkflowKind::Feasibility).version
    );
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compliance_run_requires_claims_and_reports_every_check() {
    let (service, _provider, project, _feasibility) = with_feasibility().await;
    let claims = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "claims-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let claims = wait(&service, &claims.id).await;
    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Compliance,
            "compliance-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let compliance = wait(&service, &run.id).await;
    assert_eq!(
        compliance
            .checkpoints
            .iter()
            .map(|c| c.stage)
            .collect::<Vec<_>>(),
        Stage::COMPLIANCE
    );
    // Claims text is snapshotted from the completed claims run.
    assert_eq!(
        compliance.input.upstream.claims_text,
        claims.report.clone().unwrap().markdown
    );
    let report = compliance.report.clone().unwrap().markdown;
    for label in [
        "Written description",
        "Definiteness",
        "Formalities",
        "Eligibility",
    ] {
        assert!(report.contains(label), "report must contain {label}");
    }
    assert!(report.contains("not legal advice"));
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn application_run_composes_a_full_draft_with_supplied_claims_and_prior_art() {
    let (service, provider, project, _feasibility) = with_feasibility().await;
    let claims = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "claims-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let claims = wait(&service, &claims.id).await;
    let run = service
        .start_workflow_with_prior_art(
            project.id.clone(),
            WorkflowKind::Application,
            "application-1".into(),
            authorization(),
            vec![PriorArtItem {
                reference: "US 1234567 B2".into(),
                relevance: "Synthetic reference used only for the IDS table.".into(),
            }],
        )
        .await
        .unwrap();
    let application = wait(&service, &run.id).await;
    assert_eq!(application.outcome, RunOutcome::Completed);
    assert_eq!(
        application
            .checkpoints
            .iter()
            .map(|c| c.stage)
            .collect::<Vec<_>>(),
        Stage::APPLICATION
    );
    let report = application.report.clone().unwrap().markdown;
    let order: Vec<usize> = [
        "## Abstract",
        "## Background of the Invention",
        "## Summary of the Invention",
        "## Brief Description of the Drawings",
        "## Detailed Description",
        "## Claims",
        "## Information Disclosure Statement",
    ]
    .iter()
    .map(|heading| {
        report
            .find(heading)
            .unwrap_or_else(|| panic!("report is missing {heading}"))
    })
    .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(report.contains(&claims.report.clone().unwrap().markdown));
    assert!(report.contains("US 1234567 B2"));
    // Five section generations, one per application stage.
    assert_eq!(
        provider
            .requests()
            .iter()
            .filter(|r| r.stage.kind() == WorkflowKind::Application)
            .count(),
        5
    );
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_later_feasibility_version_never_changes_an_existing_claims_snapshot() {
    let (service, _provider, project, _feasibility) = with_feasibility().await;
    let first = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "claims-1".into(),
            authorization(),
        )
        .await
        .unwrap();
    let first = wait(&service, &first.id).await;
    let snapshot = first.input.upstream.clone();
    // A newer feasibility version changes nothing for the already-created run.
    let rerun = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "feasibility-2".into(),
            authorization(),
        )
        .await
        .unwrap();
    assert_eq!(
        wait(&service, &rerun.id).await.outcome,
        RunOutcome::Completed
    );
    let reopened = service.run(first.id.clone()).await.unwrap();
    assert_eq!(reopened.input.upstream, snapshot);
    assert_eq!(reopened.report, first.report);
    // History now mixes workflows and keeps each run's own kind.
    let history = service.history(project.id).await.unwrap();
    let kinds: Vec<WorkflowKind> = history.iter().map(|r| r.input.kind).collect();
    assert_eq!(
        kinds,
        vec![
            WorkflowKind::Feasibility,
            WorkflowKind::Claims,
            WorkflowKind::Feasibility
        ]
    );
    service.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_workflow_prompt_carries_the_upstream_legal_disclaimers() {
    for kind in WorkflowKind::ALL {
        for stage in kind.stages() {
            let system = prompts::system(*stage);
            assert!(
                system.contains("legal advice"),
                "{} {} is missing the legal disclaimer",
                kind.slug(),
                stage.key()
            );
            assert!(
                system.contains("registered patent attorney"),
                "{} {} is missing the attorney notice",
                kind.slug(),
                stage.key()
            );
        }
        assert_eq!(prompts::snapshot(kind).sha256.len(), 64);
        assert!(!prompts::snapshot(kind).version.is_empty());
    }
    // Feasibility keeps its original aggregate hash, so persisted runs still authorize.
    assert_eq!(
        prompts::snapshot(WorkflowKind::Feasibility).version,
        "patentforge-7a3200c-feasibility-v1"
    );
}
