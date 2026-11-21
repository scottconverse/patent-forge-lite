use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_pfl-cli"))
}

fn interrupted_recovery(stage: usize) {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("synthetic");
    let child = cli()
        .arg("pause-fixture")
        .arg("--workspace")
        .arg(&workspace)
        .arg("--stage")
        .arg(stage.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut child = OwnedChild(child);
    let start = Instant::now();
    while !workspace.join("fixture-paused.json").is_file()
        || std::fs::read(workspace.join("synthetic-workspace.json"))
            .ok()
            .and_then(|body| serde_json::from_slice::<serde_json::Value>(&body).ok())
            .is_none_or(|marker| marker["run_id"].is_null())
    {
        assert!(
            start.elapsed() < Duration::from_secs(25),
            "fixture never reached pause"
        );
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "fixture exited before pause"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    // Terminate only the test-owned executable, without graceful shutdown.
    // While it still holds the profile lock, a second owner must refuse.
    let blocked = cli()
        .arg("resume")
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert!(
        !blocked.status.success(),
        "a CLI must not recover a profile another owner holds"
    );
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("already using"),
        "{}",
        String::from_utf8_lossy(&blocked.stderr)
    );
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    let output = cli()
        .arg("resume")
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["checkpoints"], 6);
    assert_eq!(evidence["attempts"], 2);
    assert_eq!(evidence["provider_calls_this_process"], 7 - stage);
    assert!(workspace.join("report.md").is_file());
    assert!(workspace.join("report.html").is_file());
}

#[test]
fn synthetic_import_preserves_edited_report_and_new_analysis_is_version_three() {
    let temp = tempfile::tempdir().unwrap();
    let output = cli()
        .arg("import-replay")
        .arg("--workspace")
        .arg(temp.path().join("fixture"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["version"], 3);
    assert_eq!(evidence["provider_calls_this_process"], 6);
    assert_eq!(evidence["imported_edited_report_preserved"], true);
}

#[test]
fn hard_interruption_before_first_checkpoint_recovers_in_fresh_process() {
    interrupted_recovery(1);
}

#[test]
fn hard_interruption_after_third_checkpoint_does_not_repeat_completed_stages() {
    interrupted_recovery(4);
}

#[test]
fn full_pipeline_runs_all_four_workflows_in_dependency_order() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("pipeline");
    let output = cli()
        .arg("pipeline")
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["total_stages"], 19);
    assert_eq!(evidence["provider_calls_this_process"], 19);
    assert_eq!(evidence["reopened_without_generation"], true);
    let workflows = evidence["workflows"].as_array().unwrap();
    let names: Vec<&str> = workflows
        .iter()
        .map(|w| w["workflow"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["feasibility", "claims", "compliance", "application"]
    );
    let stages: Vec<u64> = workflows
        .iter()
        .map(|w| w["stages"].as_u64().unwrap())
        .collect();
    assert_eq!(stages, [6, 4, 4, 5]);
    for name in names {
        assert!(workspace.join(format!("{name}-report.md")).is_file());
        assert!(workspace.join(format!("{name}-report.html")).is_file());
    }
}

#[test]
fn replay_persists_exports_and_refuses_nonempty_or_unmarked_workspaces() {
    let temp = tempfile::tempdir().unwrap();
    let workspace = temp.path().join("synthetic");
    let output = cli()
        .arg("replay")
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(evidence["provider_calls_this_process"], 6);
    assert_eq!(evidence["attempts"], 1);
    assert_eq!(evidence["reopened_without_generation"], true);
    let duplicate = cli()
        .arg("replay")
        .arg("--workspace")
        .arg(&workspace)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("new or empty"));
    let unrelated = temp.path().join("unrelated");
    std::fs::create_dir(&unrelated).unwrap();
    std::fs::write(unrelated.join("keep.txt"), "preserve").unwrap();
    let refused = cli()
        .arg("resume")
        .arg("--workspace")
        .arg(&unrelated)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(
        std::fs::read_to_string(unrelated.join("keep.txt")).unwrap(),
        "preserve"
    );
    assert!(
        !Path::new(&unrelated)
            .join("patent-forge-lite.sqlite")
            .exists()
    );
}
