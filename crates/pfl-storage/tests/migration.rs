use pfl_domain::*;
use pfl_storage::{
    ImportReceipt, MigrationOptions, MigrationPhase, SqliteStore, migrate_legacy_synthetic,
    migrate_legacy_synthetic_with_options, write_legacy_synthetic_fixture,
};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Map, Value};
use std::{fs, path::Path};
use tempfile::tempdir;

const ID: &str = "pfl-legacy-synthetic-v1";

fn fixture(path: &Path) -> Connection {
    write_legacy_synthetic_fixture(path).unwrap();
    Connection::open(path).unwrap()
}

fn source_rows(connection: &Connection, table: &str) -> Vec<Value> {
    let mut statement = connection
        .prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))
        .unwrap();
    let names: Vec<String> = statement.column_names().map(|s| s.to_string()).collect();
    let mut rows = statement.query([]).unwrap();
    let mut values = vec![];
    while let Some(row) = rows.next().unwrap() {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            let value = match row.get_ref(index).unwrap() {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => Value::from(v),
                ValueRef::Real(v) => Value::from(v),
                ValueRef::Text(v) => Value::String(String::from_utf8(v.to_vec()).unwrap()),
                ValueRef::Blob(_) => panic!("fixture has no binary fields"),
            };
            object.insert(name.clone(), value);
        }
        values.push(Value::Object(object));
    }
    values
}

#[test]
fn copy_first_import_projects_active_history_reports_and_every_raw_field() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("legacy.sqlite3");
    let original = fixture(&source);
    drop(original);
    let original_bytes = fs::read(&source).unwrap();
    let work = dir.path().join("migration");
    let receipt = migrate_legacy_synthetic(&source, &work).unwrap();
    assert!(!receipt.repeated);
    assert_eq!(receipt.projects_imported, 1);
    assert_eq!(receipt.runs_imported, 2);
    assert!(
        receipt
            .warnings
            .iter()
            .any(|warning| warning.contains("reconstructed"))
    );
    assert!(
        receipt
            .warnings
            .iter()
            .any(|warning| warning.contains("not generated"))
    );
    assert!(receipt.backup_path.exists());
    let active = receipt.activated_path.unwrap();
    assert_ne!(active, source);
    assert_ne!(active, receipt.backup_path);
    let store = SqliteStore::open(&active).unwrap();
    let projects = store.list_projects().unwrap();
    assert_eq!(projects[0].id.0, "synthetic-project");
    assert!(projects[0].intake.description.contains("日本語"));
    let expected_context = InventionContext {
        problem_solved: "Synthetic problem".into(),
        how_it_works: "Synthetic mechanism".into(),
        ai_components: "Synthetic AI components".into(),
        three_d_print_components: "Synthetic physical components".into(),
        what_is_novel: "Synthetic novelty".into(),
        current_alternatives: "Synthetic alternatives".into(),
        what_is_built: "Synthetic built state".into(),
        what_to_protect: "Synthetic scope".into(),
        additional_notes: "Edited notes preserved verbatim".into(),
    };
    assert_eq!(projects[0].intake.context, expected_context);
    let history = store.history(&projects[0].id).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version, 1);
    assert_eq!(history[0].outcome, RunOutcome::Completed);
    let report = history[0].report.as_ref().unwrap();
    assert_eq!(report.run_id, history[0].id);
    assert_eq!(report.version, 1);
    assert!(report.markdown.contains("Edited café report"));
    assert_eq!(history[0].checkpoints.len(), 6);
    assert_eq!(history[0].checkpoints[1].usage.input_tokens, None);
    assert_eq!(history[1].outcome, RunOutcome::Interrupted);
    assert_eq!(history[1].checkpoints.len(), 1);
    assert_eq!(history[0].input.intake.context, expected_context);
    assert_eq!(history[1].input.intake.context, expected_context);
    assert!(history[1].active_attempt().is_none());
    assert!(!history[1].input.authorization.local_inference);
    assert!(!history[1].input.authorization.paid_provider);
    assert_eq!(store.settings().unwrap().profile, SetupProfile::Full);
    assert_eq!(
        store.settings().unwrap().local_model.as_deref(),
        Some("synthetic-existing-model")
    );
    let source_db = Connection::open_with_flags(&source, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut tables = source_db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap();
    let names: Vec<String> = tables
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    for table in names {
        assert_eq!(
            store.legacy_records(ID, &table).unwrap(),
            source_rows(&source_db, &table),
            "field comparison for {table}"
        );
    }
    assert_eq!(fs::read(&source).unwrap(), original_bytes);
    let mut edited = projects[0].intake.clone();
    edited.context.problem_solved = "An explicitly saved later edit".into();
    edited.technical_field = "Separate technical field retained".into();
    store.save_intake(&projects[0].id, &edited, 20000).unwrap();
    let repeat = migrate_legacy_synthetic(&source, &work).unwrap();
    assert!(repeat.repeated);
    assert_eq!(repeat.warnings, receipt.warnings);
    assert_eq!(store.history(&projects[0].id).unwrap().len(), 2);
    assert_eq!(store.list_projects().unwrap().len(), 1);
    assert_eq!(store.project(&projects[0].id).unwrap().intake, edited);
    assert_eq!(
        store.run(&history[0].id).unwrap().input.intake.context,
        expected_context
    );
    assert_eq!(store.run(&history[0].id).unwrap().report, history[0].report);
    drop(store);
    let reopened = SqliteStore::open(&active).unwrap();
    assert_eq!(reopened.project(&projects[0].id).unwrap().intake, edited);
    assert_eq!(
        reopened.run(&history[1].id).unwrap().input.intake.context,
        expected_context
    );
}

#[test]
fn wal_committed_data_is_in_backup_and_original_main_and_wal_are_unchanged() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("legacy.sqlite3");
    let writer = fixture(&source);
    writer.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
        UPDATE InventionInput SET additionalNotes='WAL-only committed edit: café 🧪';
        INSERT INTO PatentDetail VALUES('wal-only','SYNTHETIC-WAL',NULL,NULL,NULL,'WAL-only claims');").unwrap();
    let wal_path = source.with_file_name("legacy.sqlite3-wal");
    assert!(wal_path.exists());
    let main_before = fs::read(&source).unwrap();
    let wal_before = fs::read(&wal_path).unwrap();
    let receipt = migrate_legacy_synthetic(&source, &dir.path().join("migration")).unwrap();
    let backup =
        Connection::open_with_flags(&receipt.backup_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let value: String = backup
        .query_row("SELECT additionalNotes FROM InventionInput", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(value, "WAL-only committed edit: café 🧪");
    let count: i64 = backup
        .query_row(
            "SELECT count(*) FROM PatentDetail WHERE id='wal-only'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    assert_eq!(fs::read(&source).unwrap(), main_before);
    assert_eq!(fs::read(&wal_path).unwrap(), wal_before);
    // Restore into another location using the standalone backup alone, without
    // copying live sidecars or mutating the original.
    let restore = dir.path().join("restored-legacy.sqlite3");
    drop(backup);
    fs::copy(&receipt.backup_path, &restore).unwrap();
    let restored = Connection::open_with_flags(&restore, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_eq!(
        source_rows(&restored, "InventionInput"),
        source_rows(&writer, "InventionInput")
    );
    assert_eq!(
        source_rows(&restored, "PatentDetail"),
        source_rows(&writer, "PatentDetail")
    );
}

#[test]
fn interruptions_after_backup_stage_and_activation_are_repeatable_and_preserve_original() {
    for phase in [
        MigrationPhase::Backup,
        MigrationPhase::Stage,
        MigrationPhase::Activation,
    ] {
        let dir = tempdir().unwrap();
        let source = dir.path().join("legacy.sqlite3");
        drop(fixture(&source));
        let before = fs::read(&source).unwrap();
        let work = dir.path().join("migration");
        assert!(
            migrate_legacy_synthetic_with_options(
                &source,
                &work,
                MigrationOptions {
                    interrupt_after: Some(phase)
                },
            )
            .is_err()
        );
        assert_eq!(fs::read(&source).unwrap(), before);
        assert!(work.join("legacy-backup.sqlite3").exists());
        let receipt = migrate_legacy_synthetic(&source, &work).unwrap();
        let store = SqliteStore::open(receipt.activated_path.unwrap()).unwrap();
        assert_eq!(store.list_projects().unwrap().len(), 1);
        assert_eq!(
            store
                .history(&ProjectId("synthetic-project".into()))
                .unwrap()
                .len(),
            2
        );
        assert!(migrate_legacy_synthetic(&source, &work).unwrap().repeated);
        assert_eq!(fs::read(&source).unwrap(), before);
    }
}

#[test]
fn failed_import_rolls_back_all_records_settings_and_ledger() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("legacy.sqlite3");
    let connection = fixture(&source);
    connection
        .execute(
            "UPDATE FeasibilityRun SET version=0 WHERE id='synthetic-run-v2'",
            [],
        )
        .unwrap();
    drop(connection);
    let store = SqliteStore::in_memory().unwrap();
    let error = store
        .import_legacy_synthetic(&source, &dir.path().join("backup"))
        .unwrap_err();
    assert!(matches!(error, Error::Invalid(_)));
    assert!(store.list_projects().unwrap().is_empty());
    assert!(store.legacy_records(ID, "Project").unwrap().is_empty());
    assert_eq!(store.settings().unwrap(), Settings::default());
}

#[test]
fn changed_fixture_cannot_overwrite_previous_backup_or_active_destination() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("legacy.sqlite3");
    drop(fixture(&source));
    let work = dir.path().join("migration");
    let first = migrate_legacy_synthetic(&source, &work).unwrap();
    let active = first.activated_path.unwrap();
    let active_bytes = fs::read(&active).unwrap();
    let backup_bytes = fs::read(&first.backup_path).unwrap();
    let source_db = Connection::open(&source).unwrap();
    source_db
        .execute(
            "UPDATE Project SET title='Changed after original import'",
            [],
        )
        .unwrap();
    drop(source_db);
    assert!(matches!(
        migrate_legacy_synthetic(&source, &work),
        Err(Error::Conflict(_))
    ));
    assert_eq!(fs::read(&active).unwrap(), active_bytes);
    assert_eq!(fs::read(&first.backup_path).unwrap(), backup_bytes);
}

#[test]
fn unmarked_database_and_nonempty_unowned_workspace_are_refused() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("not-a-fixture.sqlite3");
    let db = Connection::open(&source).unwrap();
    db.execute_batch("CREATE TABLE Project(id TEXT)").unwrap();
    drop(db);
    let before = fs::read(&source).unwrap();
    assert!(migrate_legacy_synthetic(&source, &dir.path().join("migration")).is_err());
    assert_eq!(fs::read(&source).unwrap(), before);
    let marked = dir.path().join("fixture.sqlite3");
    drop(fixture(&marked));
    let work = dir.path().join("occupied");
    fs::create_dir(&work).unwrap();
    fs::write(work.join("user-work.txt"), "preserve").unwrap();
    assert!(matches!(
        migrate_legacy_synthetic(&marked, &work),
        Err(Error::Conflict(_))
    ));
    assert_eq!(
        fs::read_to_string(work.join("user-work.txt")).unwrap(),
        "preserve"
    );
}

#[test]
fn old_partial_schema_keeps_unknown_fields_without_inventing_settings() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("old-fixture.sqlite3");
    let db = Connection::open(&source).unwrap();
    db.execute_batch(
        "CREATE TABLE PFLFixture(key TEXT PRIMARY KEY,value TEXT);
         INSERT INTO PFLFixture VALUES('synthetic','true'),('fixture_id','synthetic-old-schema');
         CREATE TABLE Project(id TEXT PRIMARY KEY,title TEXT,createdAt INTEGER,updatedAt INTEGER);
         INSERT INTO Project VALUES('old-id','Old synthetic title',1,2);
         CREATE TABLE InventionInput(id TEXT,projectId TEXT,title TEXT,description TEXT,problem_solved TEXT,howItWorks TEXT,technicalField TEXT);
         INSERT INTO InventionInput VALUES('old-input','old-id','Old synthetic title','Old synthetic description','Snake-case problem','Camel-case mechanism','Separate old technical field');
         CREATE TABLE AppSettings(id TEXT,ollamaModel TEXT,unknownOldField TEXT);
         INSERT INTO AppSettings VALUES('singleton','synthetic-old-model','retain me');"
    ).unwrap();
    drop(db);
    let receipt = migrate_legacy_synthetic(&source, &dir.path().join("migration")).unwrap();
    let store = SqliteStore::open(receipt.activated_path.unwrap()).unwrap();
    assert_eq!(
        store
            .project(&ProjectId("old-id".into()))
            .unwrap()
            .intake
            .title,
        "Old synthetic title"
    );
    let intake = store.project(&ProjectId("old-id".into())).unwrap().intake;
    assert_eq!(intake.context.problem_solved, "Snake-case problem");
    assert_eq!(intake.context.how_it_works, "Camel-case mechanism");
    assert_eq!(intake.context.ai_components, "");
    assert_eq!(intake.technical_field, "Separate old technical field");
    assert_eq!(
        store.settings().unwrap().local_model.as_deref(),
        Some("synthetic-old-model")
    );
    assert_eq!(
        store
            .legacy_records("synthetic-old-schema", "AppSettings")
            .unwrap()[0]["unknownOldField"],
        "retain me"
    );
}

#[test]
fn receipt_provenance_roundtrips_and_pre_warning_receipts_remain_readable() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("fixture.sqlite3");
    drop(fixture(&source));
    let before = fs::read(&source).unwrap();
    assert!(write_legacy_synthetic_fixture(&source).is_err());
    assert_eq!(fs::read(&source).unwrap(), before);
    let receipt = migrate_legacy_synthetic(&source, &dir.path().join("migration")).unwrap();
    let serialized = serde_json::to_string(&receipt).unwrap();
    assert_eq!(
        serde_json::from_str::<ImportReceipt>(&serialized).unwrap(),
        receipt
    );
    let mut old = serde_json::to_value(&receipt).unwrap();
    old.as_object_mut().unwrap().remove("warnings");
    assert!(
        serde_json::from_value::<ImportReceipt>(old)
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn actual_legacy_complete_status_and_completed_alias_preserve_checkpoints_and_edited_report() {
    // Source contract: baseline backend feasibility.service.ts marks and reads
    // run/stage status COMPLETE; COMPLETED is an additional tolerated alias.
    for status in ["COMPLETE", "COMPLETED"] {
        let dir = tempdir().unwrap();
        let source = dir.path().join("legacy.sqlite3");
        let db = fixture(&source);
        let actual: String = db
            .query_row(
                "SELECT status FROM FeasibilityRun WHERE id='synthetic-run-v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actual, "COMPLETE");
        db.execute(
            "UPDATE FeasibilityRun SET status=?1 WHERE status='COMPLETE'",
            [status],
        )
        .unwrap();
        db.execute(
            "UPDATE FeasibilityStage SET status=?1 WHERE status='COMPLETE'",
            [status],
        )
        .unwrap();
        let original_report: String = db
            .query_row(
                "SELECT finalReport FROM FeasibilityRun WHERE id='synthetic-run-v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        drop(db);
        let receipt = migrate_legacy_synthetic(&source, &dir.path().join("migration")).unwrap();
        let store = SqliteStore::open(receipt.activated_path.unwrap()).unwrap();
        let run = store.run(&RunId("synthetic-run-v1".into())).unwrap();
        assert_eq!(run.outcome, RunOutcome::Completed, "status {status}");
        assert_eq!(run.checkpoints.len(), 6, "status {status}");
        assert_eq!(run.report.unwrap().markdown, original_report);
        assert_eq!(
            store
                .run(&RunId("synthetic-run-v2".into()))
                .unwrap()
                .checkpoints
                .len(),
            1
        );
    }
}
