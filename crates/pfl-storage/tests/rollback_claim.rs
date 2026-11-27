//! The rollback claim, made checkable.
//!
//! The schema comment says an older build can still read and write this database. For a run left
//! mid-stage that means the payload's own counter may be older than the side table's, and the read
//! path takes the newer of the two rather than whichever the payload happens to say.

use pfl_domain::*;
use pfl_storage::SqliteStore;
use rusqlite::{Connection, OpenFlags};
use tempfile::tempdir;

fn project() -> Project {
    Project {
        id: ProjectId("rollback-project".into()),
        intake: Intake {
            title: "Synthetic rollback fixture".into(),
            description: "Exercises a run whose payload is older than its side table.".into(),
            technical_field: "Synthetic testing".into(),
            context: InventionContext::default(),
        },
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}

fn request() -> NewRun {
    NewRun {
        id: RunId("rollback-run".into()),
        project_id: project().id,
        idempotency_key: "rollback-start".into(),
        input: RunInput {
            kind: WorkflowKind::Feasibility,
            intake: project().intake,
            provider: ProviderSnapshot {
                kind: ProviderKind::Replay,
                model: "fixture".into(),
                endpoint: None,
            },
            prompts: PromptSnapshot {
                version: "fixture-v1".into(),
                sha256: "synthetic-hash".into(),
            },
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
            regeneration: None,
            provider_plan: Vec::new(),
        },
        rerun_of: None,
        created_at_ms: 2,
    }
}

#[test]
fn a_rollback_read_takes_the_newer_of_payload_and_side_table() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollback.sqlite3");

    // A run with two committed checkpoints: the side table now knows a sequence the payload may not.
    let newest = {
        let store = SqliteStore::open(&path).unwrap();
        store.create_project(&project()).unwrap();
        let run = store.create_run(&request()).unwrap();
        let attempt = store.begin_attempt(&run.id, 3).unwrap();
        for stage in [Stage::TechnicalRestatement, Stage::PriorArt] {
            store
                .commit_checkpoint(
                    &run.id,
                    &attempt.id,
                    &Checkpoint {
                        stage,
                        attempt_id: attempt.id.clone(),
                        text: format!("saved stage {}", stage.number()),
                        provider: None,
                        usage: Usage::default(),
                        completed_at_ms: 10 + stage.number() as u64,
                    },
                )
                .unwrap();
        }
        store.run(&run.id).unwrap().last_sequence
    };
    assert!(newest > 0, "the run must have a sequence to be meaningful");

    // An older build writes the payload without knowing the side table: its counter is 0.
    {
        let connection =
            Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
        let payload: String = connection
            .query_row(
                "SELECT payload FROM runs WHERE id=?1",
                ["rollback-run"],
                |r| r.get(0),
            )
            .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        value["last_sequence"] = serde_json::json!(0);
        connection
            .execute(
                "UPDATE runs SET payload=?1 WHERE id=?2",
                rusqlite::params![value.to_string(), "rollback-run"],
            )
            .unwrap();
    }

    // The read path reports the newer value, not the payload's stale one.
    let store = SqliteStore::open(&path).unwrap();
    let run = store.run(&RunId("rollback-run".into())).unwrap();
    assert_eq!(
        run.last_sequence, newest,
        "a run left mid-stage must be read at the side table's sequence"
    );
}
