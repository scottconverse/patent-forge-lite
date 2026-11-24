use pfl_domain::*;
use pfl_storage::{SqliteStore, migrate_legacy_synthetic, write_legacy_synthetic_fixture};
use pfl_workflow::{ReplayProvider, WorkflowService, export_html, export_markdown};
use std::sync::Arc;
use tokio::runtime::Handle;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn imported_edited_report_remains_readable_exportable_and_not_regenerated() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("authored-synthetic-legacy.sqlite3");
    write_legacy_synthetic_fixture(&source).unwrap();
    let receipt = migrate_legacy_synthetic(&source, &dir.path().join("new-migration")).unwrap();
    let path = receipt.activated_path.unwrap();
    let provider = Arc::new(ReplayProvider::synthetic());
    let service = WorkflowService::new(
        Arc::new(SqliteStore::open(&path).unwrap()),
        provider.clone(),
        Handle::current(),
    );
    let id = RunId("synthetic-run-v1".into());
    let run = service.run(id.clone()).await.unwrap();
    assert_eq!(run.outcome, RunOutcome::Completed);
    let report = service.report(id.clone()).await.unwrap();
    assert!(report.markdown.contains("Edited café report"));
    let original_stage6 = &run
        .checkpoints
        .iter()
        .find(|cp| cp.stage == Stage::FinalReport)
        .unwrap()
        .text;
    assert_ne!(
        original_stage6, &report.markdown,
        "fixture must exercise a saved edit rather than an identical checkpoint"
    );
    let md = export_markdown(&run).unwrap();
    let html = export_html(&run).unwrap();
    assert!(md.contains(&report.markdown));
    assert!(md.contains("reconstructed from intake available at import"));
    assert!(html.contains("Edited café report") && html.contains("日本語 🧪"));
    let md_path = dir.path().join("historical-report.md");
    let html_path = dir.path().join("historical-report.html");
    std::fs::write(&md_path, &md).unwrap();
    std::fs::write(&html_path, &html).unwrap();
    assert_eq!(std::fs::read_to_string(md_path).unwrap(), md);
    assert_eq!(std::fs::read_to_string(html_path).unwrap(), html);
    // Historical reconstructed input is not authorization to regenerate.
    assert!(
        service
            .resume(
                RunId("synthetic-run-v2".into()),
                Authorization {
                    synthetic_data: true,
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    assert_eq!(provider.request_count(), 0);
    service.shutdown().await.unwrap();
    drop(service);
    let reopened = WorkflowService::new(
        Arc::new(SqliteStore::open(path).unwrap()),
        provider.clone(),
        Handle::current(),
    );
    assert_eq!(reopened.report(id.clone()).await.unwrap(), report);
    assert_eq!(reopened.run(id).await.unwrap().checkpoints, run.checkpoints);
    assert_eq!(provider.request_count(), 0);
}
