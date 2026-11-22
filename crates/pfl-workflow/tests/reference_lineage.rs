//! Reference lineage: a reference set supplied to feasibility must reach every
//! downstream workflow and the exported Information Disclosure Statement.
//!
//! The IDS is assembled from the recorded set — never from model prose — so the
//! test asserts the exact rows the operator supplied.
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::sync::Arc;

fn intake() -> Intake {
    Intake {
        title: "Reference lineage fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. A fictional ",
            "container carries a temperature sensor sampled by a controller that drives one blue ",
            "indicator and reports an unavailable sensor instead of inventing a reading. A ",
            "removable fixture supplies deterministic synthetic readings for restart tests. This ",
            "paragraph exists only to satisfy the explicit word minimum and to exercise reference ",
            "lineage deterministically with recorded responses, Unicode café, and no real invention ",
            "data of any kind whatsoever.",
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

fn references() -> Vec<PriorArtItem> {
    vec![
        PriorArtItem {
            reference: "US-2023-0312345-B2".into(),
            relevance: "Temperature sensing container with integrated indicators".into(),
            title: "Container with integrated temperature indicator".into(),
            source: "Manual".into(),
            publication_number: "US 2023/0312345 B2".into(),
            filing_date: "2023-03-14".into(),
            grant_date: "2024-01-09".into(),
            assignee: "Example Sensors Inc.".into(),
            cpc: "G01K 1/02".into(),
            url: "https://patents.example/us-20230312345".into(),
            verified: true,
        },
        PriorArtItem {
            reference: "EP-1234567-A1".into(),
            relevance: "Container with a temperature sensor using light to indicate ranges".into(),
            source: "Manual".into(),
            verified: true,
            ..Default::default()
        },
    ]
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
async fn supplied_references_reach_every_downstream_workflow_and_the_ids() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let project = service.create_project(intake()).await.unwrap();

    // Feasibility with an explicit reference set.
    let feasibility = service
        .start_workflow_with_prior_art(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "refs-f1".into(),
            auth(),
            references(),
        )
        .await
        .unwrap();
    let feasibility = wait(&service, &feasibility.id).await;
    assert_eq!(feasibility.outcome, RunOutcome::Completed);
    assert_eq!(feasibility.input.upstream.prior_art.len(), 2);

    // Claims inherits them without re-entry.
    let claims = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Claims,
            "refs-c1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(
        claims.input.upstream.prior_art,
        references(),
        "claims must inherit the recorded reference set verbatim"
    );
    let claims = wait(&service, &claims.id).await;
    assert_eq!(claims.outcome, RunOutcome::Completed);

    // Compliance inherits them too.
    let compliance = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Compliance,
            "refs-k1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(compliance.input.upstream.prior_art, references());
    let compliance = wait(&service, &compliance.id).await;
    assert_eq!(compliance.outcome, RunOutcome::Completed);

    // The application drafts a deterministic IDS from those same records.
    let application = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Application,
            "refs-a1".into(),
            auth(),
        )
        .await
        .unwrap();
    assert_eq!(application.input.upstream.prior_art, references());
    let application = wait(&service, &application.id).await;
    assert_eq!(application.outcome, RunOutcome::Completed);

    let report = service.report(application.id.clone()).await.unwrap();
    let ids = report
        .markdown
        .split("Information Disclosure Statement")
        .nth(1)
        .expect("the report contains an IDS section");

    for expected in [
        "US-2023-0312345-B2",
        "Container with integrated temperature indicator",
        "Filed 2023-03-14; granted 2024-01-09",
        "Example Sensors Inc.",
        "EP-1234567-A1",
        "Verified",
    ] {
        assert!(
            ids.contains(expected),
            "the IDS must contain the recorded record ({expected}); got:\n{ids}"
        );
    }
    // Two records, two rows — no invented additions and no duplicates.
    let rows = ids
        .lines()
        .filter(|line| {
            line.starts_with("| ") && !line.contains("---") && !line.contains("Reference |")
        })
        .count();
    assert_eq!(rows, 2, "the IDS lists exactly the supplied records");
}
