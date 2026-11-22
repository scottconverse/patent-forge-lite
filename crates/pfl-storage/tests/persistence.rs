use pfl_domain::*;
use pfl_storage::SqliteStore;
use rusqlite::Connection;
use std::sync::{Arc, Barrier};
use tempfile::tempdir;

fn project() -> Project {
    Project {
        id: ProjectId("synthetic-project".into()),
        intake: Intake {
            title: "Synthetic café 🧪".into(),
            description: "日本語, edited description".into(),
            technical_field: "Synthetic testing".into(),
            context: InventionContext {
                problem_solved: "Synthetic problem".into(),
                how_it_works: "Synthetic mechanism".into(),
                ai_components: "Synthetic AI components".into(),
                three_d_print_components: "Synthetic physical components".into(),
                what_is_novel: "Synthetic novelty".into(),
                current_alternatives: "Synthetic alternatives".into(),
                what_is_built: "Synthetic built state".into(),
                what_to_protect: "Synthetic scope".into(),
                additional_notes: "Edited notes preserved verbatim".into(),
            },
        },
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}
fn request() -> NewRun {
    NewRun {
        id: RunId("synthetic-run".into()),
        project_id: project().id,
        idempotency_key: "start-1".into(),
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
                sha256: "synthetic-hash-preserved".into(),
            },
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
            regeneration: None,
        },
        rerun_of: None,
        created_at_ms: 2,
    }
}
fn prepared(store: &SqliteStore) -> (RunId, Attempt) {
    store.create_project(&project()).unwrap();
    let run = store.create_run(&request()).unwrap();
    let attempt = store.begin_attempt(&run.id, 3).unwrap();
    (run.id, attempt)
}
fn checkpoint(attempt: &Attempt, stage: Stage) -> Checkpoint {
    Checkpoint {
        stage,
        attempt_id: attempt.id.clone(),
        text: format!("Synthetic stage {} café 🧪", stage.number()),
        usage: Usage {
            input_tokens: Some(stage.number() as u64),
            output_tokens: None,
        },
        completed_at_ms: 10 + stage.number() as u64,
    }
}

#[test]
fn failed_attempt_atomically_persists_only_fixed_sanitized_terminal_notice() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let (id, attempt) = prepared(&store);
    store
        .commit_checkpoint(
            &id,
            &attempt.id,
            &checkpoint(&attempt, Stage::TechnicalRestatement),
        )
        .unwrap();
    let before = store.run(&id).unwrap().last_sequence;
    let failed = store
        .finish_attempt(&id, &attempt.id, RunOutcome::Failed, None, 100)
        .unwrap();
    assert_eq!(failed.last_sequence, before + 1);
    assert_eq!(failed.outcome, RunOutcome::Failed);
    assert_eq!(failed.checkpoints.len(), 1);
    assert_eq!(failed.attempts[0].finished_at_ms, Some(100));
    drop(store);
    let reopened = SqliteStore::open(&path).unwrap();
    let events = reopened
        .events_after(&Subscribe {
            run_id: id.clone(),
            after_sequence: before,
        })
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].event,
        EventKind::OutcomeChanged {
            outcome: RunOutcome::Failed,
            message: Some(
                "Run failed. Completed stages were preserved; review settings before resuming."
                    .into()
            ),
        }
    );
    let notice = serde_json::to_string(&events[0].event).unwrap();
    assert!(!notice.contains("synthetic-hash-preserved"));
    assert!(!notice.contains("日本語"));
    assert!(!notice.contains("provider_error"));
    assert_eq!(
        reopened
            .finish_attempt(&id, &attempt.id, RunOutcome::Failed, None, 101)
            .unwrap()
            .last_sequence,
        failed.last_sequence
    );
}

#[test]
fn project_edits_settings_and_run_inputs_survive_reopening() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    assert_eq!(store.settings().unwrap(), Settings::default());
    store.create_project(&project()).unwrap();
    assert_eq!(store.create_project(&project()).unwrap(), project());
    let run = store.create_run(&request()).unwrap();
    let mut edit = project().intake;
    edit.description = "A second Unicode edit: café 日本語 🧪".into();
    edit.context.additional_notes = "A later context edit must not change run snapshots".into();
    store.save_intake(&project().id, &edit, 20).unwrap();
    store.save_intake(&project().id, &edit, 21).unwrap();
    let settings = Settings {
        profile: SetupProfile::Full,
        local_model: Some("fixture-existing".into()),
        local_endpoint: None,
        research_endpoint: None,
        profile_chosen: true,
    };
    store.save_settings(&settings).unwrap();
    drop(store);
    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(reopened.project(&project().id).unwrap().intake, edit);
    assert_eq!(reopened.intake_history(&project().id).unwrap().len(), 2);
    assert_eq!(
        reopened.intake_history(&project().id).unwrap()[0]
            .intake
            .context,
        project().intake.context
    );
    assert_eq!(reopened.settings().unwrap(), settings);
    assert_eq!(reopened.run(&run.id).unwrap().input, request().input);
    assert_eq!(reopened.list_projects().unwrap().len(), 1);
}

#[test]
fn repeated_start_is_idempotent_but_changed_input_conflicts() {
    let store = SqliteStore::in_memory().unwrap();
    store.create_project(&project()).unwrap();
    let first = store.create_run(&request()).unwrap();
    let mut retry = request();
    retry.id = RunId::new();
    retry.created_at_ms = 100;
    assert_eq!(store.create_run(&retry).unwrap(), first);
    retry.input.intake.description.push_str(" changed");
    assert!(matches!(store.create_run(&retry), Err(Error::Conflict(_))));
    retry.idempotency_key = "rerun".into();
    retry.rerun_of = Some(first.id.clone());
    let second = store.create_run(&retry).unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(store.history(&project().id).unwrap().len(), 2);
}

#[test]
fn concurrent_connections_start_one_run_and_only_one_attempt() {
    let dir = tempdir().unwrap();
    let path = Arc::new(dir.path().join("lite.sqlite3"));
    let store = SqliteStore::open(path.as_path()).unwrap();
    store.create_project(&project()).unwrap();
    let barrier = Arc::new(Barrier::new(8));
    let joins: Vec<_> = (0..8)
        .map(|_| {
            let barrier = barrier.clone();
            let path = path.clone();
            std::thread::spawn(move || {
                let store = SqliteStore::open(path.as_path()).unwrap();
                let mut request = request();
                request.id = RunId::new();
                barrier.wait();
                let run = store.create_run(&request).unwrap();
                let started = store.begin_attempt(&run.id, 3).is_ok();
                (run.id, started)
            })
        })
        .collect();
    let results: Vec<_> = joins.into_iter().map(|j| j.join().unwrap()).collect();
    assert!(results.iter().all(|v| v.0 == results[0].0));
    assert_eq!(results.iter().filter(|v| v.1).count(), 1);
    assert_eq!(store.history(&project().id).unwrap().len(), 1);
    assert_eq!(store.run(&results[0].0).unwrap().attempts.len(), 1);
}

#[test]
fn checkpoint_usage_and_event_are_atomic_and_retry_does_not_duplicate() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let (id, attempt) = prepared(&store);
    let injector = Connection::open(&path).unwrap();
    injector
        .execute_batch(
            "CREATE TRIGGER fail_checkpoint BEFORE INSERT ON events
         WHEN json_extract(NEW.payload,'$.event.kind')='checkpoint_saved'
         BEGIN SELECT RAISE(ABORT,'synthetic disk-write failure'); END;",
        )
        .unwrap();
    let cp = checkpoint(&attempt, Stage::TechnicalRestatement);
    assert!(store.commit_checkpoint(&id, &attempt.id, &cp).is_err());
    let failed = store.run(&id).unwrap();
    assert!(failed.checkpoints.is_empty());
    assert_eq!(failed.last_sequence, 2);
    injector
        .execute_batch("DROP TRIGGER fail_checkpoint")
        .unwrap();
    let event = store.commit_checkpoint(&id, &attempt.id, &cp).unwrap();
    assert_eq!(
        event,
        store.commit_checkpoint(&id, &attempt.id, &cp).unwrap()
    );
    let snapshot = store.run(&id).unwrap();
    assert_eq!(snapshot.checkpoints, vec![cp.clone()]);
    assert_eq!(snapshot.last_sequence, 3);
    assert_eq!(snapshot.checkpoints[0].usage.output_tokens, None);
    let mut changed = cp;
    changed.text.push_str("changed");
    assert!(matches!(
        store.commit_checkpoint(&id, &attempt.id, &changed),
        Err(Error::Conflict(_))
    ));
}

#[test]
fn interrupted_before_first_stage_reopens_and_resumes_same_run() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let (id, first) = prepared(&store);
    drop(store); // no finish or checkpoint: represents loss of the process owner.
    let store = SqliteStore::open(&path).unwrap();
    // Opening and read-only navigation must not mutate or auto-recover.
    assert_eq!(store.run(&id).unwrap().outcome, RunOutcome::Running);
    assert_eq!(store.recover_interrupted(30).unwrap(), vec![id.clone()]);
    assert!(store.recover_interrupted(31).unwrap().is_empty());
    let second = store.begin_attempt(&id, 32).unwrap();
    assert_eq!(second.number, 2);
    assert_ne!(first.id, second.id);
    assert!(
        store
            .commit_checkpoint(
                &id,
                &first.id,
                &checkpoint(&first, Stage::TechnicalRestatement)
            )
            .is_err()
    );
    assert!(
        store
            .append_event(
                &id,
                &first.id,
                EventKind::TextDelta {
                    stage: Stage::TechnicalRestatement,
                    text: "late".into()
                },
                33
            )
            .is_err()
    );
    assert!(store.set_cancelling(&id, &first.id, 33).is_err());
    assert!(
        store
            .finish_attempt(&id, &first.id, RunOutcome::Failed, None, 33)
            .is_err()
    );
    store
        .commit_checkpoint(
            &id,
            &second.id,
            &checkpoint(&second, Stage::TechnicalRestatement),
        )
        .unwrap();
    assert_eq!(store.run(&id).unwrap().checkpoints.len(), 1);
}

#[test]
fn later_interruption_preserves_completed_checkpoints_and_final_report() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let (id, first) = prepared(&store);
    for stage in Stage::ALL.into_iter().take(2) {
        store
            .commit_checkpoint(&id, &first.id, &checkpoint(&first, stage))
            .unwrap();
    }
    drop(store);
    let store = SqliteStore::open(&path).unwrap();
    store.recover_interrupted(50).unwrap();
    let second = store.begin_attempt(&id, 51).unwrap();
    for stage in Stage::ALL.into_iter().skip(2) {
        store
            .commit_checkpoint(&id, &second.id, &checkpoint(&second, stage))
            .unwrap();
    }
    let report = SavedReport {
        run_id: id.clone(),
        version: 1,
        markdown: "# Synthetic report\n\ncafé 日本語 🧪".into(),
    };
    let completed = store
        .finish_attempt(&id, &second.id, RunOutcome::Completed, Some(&report), 100)
        .unwrap();
    assert_eq!(
        completed,
        store
            .finish_attempt(&id, &second.id, RunOutcome::Completed, Some(&report), 101)
            .unwrap()
    );
    assert!(store.begin_attempt(&id, 102).is_err());
    drop(store);
    let store = SqliteStore::open(&path).unwrap();
    let reopened = store.run(&id).unwrap();
    assert_eq!(reopened.report, Some(report));
    assert_eq!(reopened.checkpoints.len(), 6);
    assert_eq!(reopened.checkpoints[0].attempt_id, first.id);
    assert_eq!(reopened.checkpoints[2].attempt_id, second.id);
    let before = reopened.last_sequence;
    let events = store
        .events_after(&Subscribe {
            run_id: id.clone(),
            after_sequence: 0,
        })
        .unwrap();
    assert_eq!(
        events.iter().map(|e| e.sequence).collect::<Vec<_>>(),
        (1..=before).collect::<Vec<_>>()
    );
    assert!(
        store
            .events_after(&Subscribe {
                run_id: id.clone(),
                after_sequence: before
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.run(&id).unwrap().last_sequence, before);
}

#[test]
fn cancelling_is_idempotent_blocks_late_work_and_keeps_usage() {
    let store = SqliteStore::in_memory().unwrap();
    let (id, attempt) = prepared(&store);
    store
        .commit_checkpoint(
            &id,
            &attempt.id,
            &checkpoint(&attempt, Stage::TechnicalRestatement),
        )
        .unwrap();
    let cancelled = store.set_cancelling(&id, &attempt.id, 40).unwrap();
    assert_eq!(
        cancelled,
        store.set_cancelling(&id, &attempt.id, 41).unwrap()
    );
    assert!(
        store
            .append_event(
                &id,
                &attempt.id,
                EventKind::TextDelta {
                    stage: Stage::PriorArt,
                    text: "late token".into(),
                },
                41
            )
            .is_err()
    );
    assert!(
        store
            .append_event(
                &id,
                &attempt.id,
                EventKind::StageStarted {
                    stage: Stage::PriorArt
                },
                41
            )
            .is_err()
    );
    assert!(
        store
            .commit_checkpoint(
                &id,
                &attempt.id,
                &checkpoint(&attempt, Stage::TechnicalRestatement)
            )
            .is_err()
    );
    assert!(
        store
            .commit_checkpoint(&id, &attempt.id, &checkpoint(&attempt, Stage::PriorArt))
            .is_err()
    );
    let end = store
        .finish_attempt(&id, &attempt.id, RunOutcome::Cancelled, None, 42)
        .unwrap();
    assert_eq!(end.checkpoints.len(), 1);
    assert_eq!(end.checkpoints[0].usage.input_tokens, Some(1));
    assert_eq!(
        end,
        store
            .finish_attempt(&id, &attempt.id, RunOutcome::Cancelled, None, 43)
            .unwrap()
    );
}

#[test]
fn invalid_report_and_out_of_order_stage_roll_back() {
    let store = SqliteStore::in_memory().unwrap();
    let (id, attempt) = prepared(&store);
    assert!(
        store
            .commit_checkpoint(&id, &attempt.id, &checkpoint(&attempt, Stage::PriorArt))
            .is_err()
    );
    assert!(
        store
            .finish_attempt(&id, &attempt.id, RunOutcome::Completed, None, 20)
            .is_err()
    );
    let run = store.run(&id).unwrap();
    assert_eq!(run.outcome, RunOutcome::Running);
    assert_eq!(run.last_sequence, 2);
    assert!(run.checkpoints.is_empty());
    assert!(
        store
            .append_event(
                &id,
                &attempt.id,
                EventKind::OutcomeChanged {
                    outcome: RunOutcome::Completed,
                    message: None
                },
                21
            )
            .is_err()
    );
}

#[test]
fn foreign_database_is_never_initialized_or_modified() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("not-lite.sqlite3");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE Original(value TEXT); INSERT INTO Original VALUES('preserve me')",
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(SqliteStore::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn bundled_sqlite_has_functional_fts5() {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE VIRTUAL TABLE report_search USING fts5(content);
         INSERT INTO report_search VALUES('synthetic marker café');",
    )
    .unwrap();
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM report_search WHERE report_search MATCH 'marker'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn concurrent_cancel_checkpoint_race_is_serialized_and_terminal_usage_is_consistent() {
    for _ in 0..16 {
        let store = Arc::new(SqliteStore::in_memory().unwrap());
        let (id, attempt) = prepared(&store);
        let barrier = Arc::new(Barrier::new(2));
        let thread_store = store.clone();
        let thread_barrier = barrier.clone();
        let thread_id = id.clone();
        let thread_attempt = attempt.clone();
        let save = std::thread::spawn(move || {
            thread_barrier.wait();
            thread_store
                .commit_checkpoint(
                    &thread_id,
                    &thread_attempt.id,
                    &checkpoint(&thread_attempt, Stage::TechnicalRestatement),
                )
                .is_ok()
        });
        barrier.wait();
        store.set_cancelling(&id, &attempt.id, 50).unwrap();
        let saved = save.join().unwrap();
        let end = store
            .finish_attempt(&id, &attempt.id, RunOutcome::Cancelled, None, 51)
            .unwrap();
        assert_eq!(end.outcome, RunOutcome::Cancelled);
        assert_eq!(end.checkpoints.len(), usize::from(saved));
        let checkpoint_events = store
            .events_after(&Subscribe {
                run_id: id,
                after_sequence: 0,
            })
            .unwrap()
            .iter()
            .filter(|e| matches!(e.event, EventKind::CheckpointSaved { .. }))
            .count();
        assert_eq!(checkpoint_events, usize::from(saved));
    }
}

#[test]
fn terminal_report_event_failure_cannot_publish_report_or_finish_attempt() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("lite.sqlite3");
    let store = SqliteStore::open(&path).unwrap();
    let (id, attempt) = prepared(&store);
    for stage in Stage::ALL {
        store
            .commit_checkpoint(&id, &attempt.id, &checkpoint(&attempt, stage))
            .unwrap();
    }
    let injector = Connection::open(&path).unwrap();
    injector
        .execute_batch(
            "CREATE TRIGGER fail_finish BEFORE INSERT ON events
         WHEN json_extract(NEW.payload,'$.event.kind')='outcome_changed'
         BEGIN SELECT RAISE(ABORT,'synthetic report commit failure'); END;",
        )
        .unwrap();
    let report = SavedReport {
        run_id: id.clone(),
        version: 1,
        markdown: "Synthetic report".into(),
    };
    let before = store.run(&id).unwrap();
    assert!(
        store
            .finish_attempt(&id, &attempt.id, RunOutcome::Completed, Some(&report), 100)
            .is_err()
    );
    assert_eq!(store.run(&id).unwrap(), before);
    injector.execute_batch("DROP TRIGGER fail_finish").unwrap();
    assert_eq!(
        store
            .finish_attempt(&id, &attempt.id, RunOutcome::Completed, Some(&report), 100)
            .unwrap()
            .report,
        Some(report)
    );
}

// Launched by the parent test, not a provider or additional Cargo invocation.
#[test]
fn abrupt_exit_child() {
    let Some(path) = std::env::var_os("PFL_STORAGE_CRASH_DB") else {
        return;
    };
    let store = SqliteStore::open(std::path::PathBuf::from(&path)).unwrap();
    let (id, attempt) = prepared(&store);
    if std::env::var_os("PFL_STORAGE_CRASH_LATER").is_some() {
        store
            .commit_checkpoint(
                &id,
                &attempt.id,
                &checkpoint(&attempt, Stage::TechnicalRestatement),
            )
            .unwrap();
    }
    let uncommitted = Connection::open(std::path::PathBuf::from(path)).unwrap();
    uncommitted
        .execute_batch("BEGIN IMMEDIATE; DELETE FROM events")
        .unwrap();
    // Skip Rust destructors, leaving an uncommitted write and open WAL handles.
    std::process::exit(23);
}

#[test]
fn real_child_exit_rolls_back_uncommitted_events_and_recovers_before_and_after_stage_one() {
    for later in [false, true] {
        let dir = tempdir().unwrap();
        let path = dir.path().join("lite.sqlite3");
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "abrupt_exit_child", "--nocapture"])
            .env("PFL_STORAGE_CRASH_DB", &path);
        if later {
            command.env("PFL_STORAGE_CRASH_LATER", "1");
        } else {
            command.env_remove("PFL_STORAGE_CRASH_LATER");
        }
        let output = command.output().unwrap();
        assert_eq!(
            output.status.code(),
            Some(23),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let store = SqliteStore::open(&path).unwrap();
        let id = request().id;
        let run = store.run(&id).unwrap();
        assert_eq!(run.checkpoints.len(), usize::from(later));
        assert_eq!(
            store
                .events_after(&Subscribe {
                    run_id: id.clone(),
                    after_sequence: 0
                })
                .unwrap()
                .len() as u64,
            run.last_sequence
        );
        store.recover_interrupted(50).unwrap();
        assert_eq!(store.begin_attempt(&id, 51).unwrap().number, 2);
    }
}
