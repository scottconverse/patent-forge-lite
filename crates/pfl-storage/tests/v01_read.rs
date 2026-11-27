//! A database in the released v0.1.0 shape, read through the read-only path.
//!
//! v0.1.0 had no `run_sequences` table: `runs` was five columns wide and the event sequence was
//! derived from the payload. Both read queries used to `LEFT JOIN run_sequences`, which is a SQL
//! error against a database that never had the table, so `inspect` could not open an old file.

use pfl_domain::*;
use pfl_storage::SqliteStore;
use rusqlite::{Connection, OpenFlags};
use tempfile::tempdir;

fn project() -> Project {
    Project {
        id: ProjectId("old-project".into()),
        intake: Intake {
            title: "Synthetic v0.1.0 fixture".into(),
            description: "Built by the test, not from the current schema.".into(),
            technical_field: "Synthetic testing".into(),
            context: InventionContext::default(),
        },
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}

fn request() -> NewRun {
    NewRun {
        id: RunId("old-run".into()),
        project_id: project().id,
        idempotency_key: "old-start".into(),
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

/// Build the file: a real run, then the table removed and the version set back to 1, which is the
/// shape a released v0.1.0 build wrote.
fn fixture(path: &std::path::Path) -> u64 {
    let store = SqliteStore::open(path).unwrap();
    store.create_project(&project()).unwrap();
    store.create_run(&request()).unwrap();
    let expected = store.run(&RunId("old-run".into())).unwrap().last_sequence;
    drop(store);

    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    connection
        .execute_batch("DROP TABLE run_sequences; UPDATE schema_migrations SET version=1;")
        .unwrap();
    drop(connection);

    // The fixture really is old-shaped: no side table, runs still five columns wide.
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='run_sequences'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0, "the fixture must have no run_sequences table");
    let columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runs')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(columns, 5, "a v0.1.0 runs table is five columns wide");
    expected
}

#[test]
fn a_v0_1_0_database_reads_read_only_and_is_not_modified() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("v010.sqlite3");
    let expected = fixture(&path);
    let before = std::fs::read(&path).unwrap();

    let store = SqliteStore::open_read_only(&path).unwrap();
    let run = store.run(&RunId("old-run".into())).unwrap();
    assert_eq!(
        run.last_sequence, expected,
        "with no side table the sequence is the payload's own"
    );
    let history = store.history(&project().id).unwrap();
    assert_eq!(history.len(), 1, "history reads through the same rule");
    assert_eq!(history[0].last_sequence, expected);
    drop(store);

    let after = std::fs::read(&path).unwrap();
    assert_eq!(before, after, "opening read-only must not modify the file");
}

#[test]
fn the_read_write_path_migrates_the_same_file_and_agrees() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("v010-migrated.sqlite3");
    let expected = fixture(&path);

    let store = SqliteStore::open(&path).unwrap();
    let run = store.run(&RunId("old-run".into())).unwrap();
    assert_eq!(
        run.last_sequence, expected,
        "after migrating, the sequence still reads as the payload's"
    );
    assert_eq!(store.history(&project().id).unwrap().len(), 1);
}
