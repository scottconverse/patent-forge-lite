//! SQLite persistence for explicitly selected Patent Forge Lite databases.
//! No constructor discovers, opens, or imports a PatentForge installation.
use pfl_domain::*;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{path::Path, sync::Mutex, time::Duration};

mod migration;
pub use migration::{
    ImportReceipt, LEGACY_SYNTHETIC_FIXTURE_SQL, MigrationOptions, MigrationPhase,
    migrate_legacy_synthetic, migrate_legacy_synthetic_with_options,
    write_legacy_synthetic_fixture,
};

const APPLICATION_ID: i64 = 0x5046_4c31;

pub struct SqliteStore {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntakeRevision {
    pub revision: u64,
    pub intake: Intake,
    pub at_ms: u64,
}

fn db(error: impl std::fmt::Display) -> Error {
    Error::Storage(error.to_string())
}
fn encode(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(db)
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(db)
}
fn integer(value: u64) -> Result<i64> {
    value
        .try_into()
        .map_err(|_| Error::Invalid("integer exceeds SQLite range".into()))
}
fn read_json<T: DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    id: &str,
) -> Result<Option<T>> {
    let value: Option<String> = connection
        .query_row(sql, [id], |row| row.get(0))
        .optional()
        .map_err(db)?;
    value.map(|s| decode(&s)).transpose()
}
fn load_run(connection: &Connection, id: &RunId) -> Result<RunSnapshot> {
    read_json(connection, "SELECT payload FROM runs WHERE id=?1", &id.0)?
        .ok_or_else(|| Error::NotFound(format!("run {id}")))
}
fn load_project(connection: &Connection, id: &ProjectId) -> Result<Project> {
    read_json(
        connection,
        "SELECT payload FROM projects WHERE id=?1",
        &id.0,
    )?
    .ok_or_else(|| Error::NotFound(format!("project {id}")))
}
fn save_run(transaction: &Transaction<'_>, run: &RunSnapshot) -> Result<()> {
    transaction
        .execute(
            "UPDATE runs SET payload=?2 WHERE id=?1",
            params![run.id.0, encode(run)?],
        )
        .map_err(db)?;
    Ok(())
}
fn active(run: &RunSnapshot, attempt: &AttemptId) -> Result<()> {
    if run.active_attempt().map(|a| &a.id) != Some(attempt) {
        return Err(Error::Conflict("stale or finished attempt".into()));
    }
    Ok(())
}
fn push_event(
    transaction: &Transaction<'_>,
    run: &mut RunSnapshot,
    attempt: Option<AttemptId>,
    kind: EventKind,
    at_ms: u64,
) -> Result<RunEvent> {
    let sequence = run
        .last_sequence
        .checked_add(1)
        .ok_or_else(|| db("sequence exhausted"))?;
    let event = RunEvent {
        run_id: run.id.clone(),
        attempt_id: attempt,
        sequence,
        at_ms,
        event: kind,
    };
    transaction
        .execute(
            "INSERT INTO events(run_id,sequence,payload) VALUES(?1,?2,?3)",
            params![run.id.0, integer(sequence)?, encode(&event)?],
        )
        .map_err(db)?;
    run.last_sequence = sequence;
    Ok(event)
}

impl SqliteStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(db)?;
        Self::initialize(connection)
    }

    pub fn in_memory() -> Result<Self> {
        Self::initialize(Connection::open_in_memory().map_err(db)?)
    }

    fn initialize(mut connection: Connection) -> Result<Self> {
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(db)?;
        let application_id: i64 = connection
            .query_row("PRAGMA application_id", [], |r| r.get(0))
            .map_err(db)?;
        let table_count: i64 = connection.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [], |r| r.get(0),
        ).map_err(db)?;
        if application_id != APPLICATION_ID && (application_id != 0 || table_count != 0) {
            return Err(Error::Invalid(
                "not a Patent Forge Lite database; explicit import is required".into(),
            ));
        }
        if application_id == APPLICATION_ID {
            let version: i64 = connection
                .query_row(
                    "SELECT coalesce(max(version),0) FROM schema_migrations",
                    [],
                    |r| r.get(0),
                )
                .map_err(db)?;
            if version != 1 {
                return Err(Error::Invalid(format!(
                    "unsupported Lite schema version {version}"
                )));
            }
        }
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
            )
            .map_err(db)?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db)?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS intake_revisions(
               project_id TEXT NOT NULL REFERENCES projects(id),revision INTEGER NOT NULL,
               payload TEXT NOT NULL,PRIMARY KEY(project_id,revision));
             CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),payload TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS runs(
               id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),
               version INTEGER NOT NULL CHECK(version>0),idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,
               UNIQUE(project_id,version),UNIQUE(project_id,idempotency_key));
             CREATE TABLE IF NOT EXISTS events(
               run_id TEXT NOT NULL REFERENCES runs(id),sequence INTEGER NOT NULL CHECK(sequence>0),
               payload TEXT NOT NULL,PRIMARY KEY(run_id,sequence));
             CREATE TABLE IF NOT EXISTS legacy_imports(
               identity TEXT PRIMARY KEY,snapshot TEXT NOT NULL,receipt TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS legacy_records(
               identity TEXT NOT NULL REFERENCES legacy_imports(identity),table_name TEXT NOT NULL,
               ordinal INTEGER NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(identity,table_name,ordinal));
             INSERT OR IGNORE INTO schema_migrations(version) VALUES(1);"
        ).map_err(db)?;
        transaction
            .pragma_update(None, "application_id", APPLICATION_ID)
            .map_err(db)?;
        transaction.commit().map_err(db)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn transaction<T>(&self, work: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| db("database mutex poisoned"))?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db)?;
        let result = work(&transaction)?;
        transaction.commit().map_err(db)?;
        Ok(result)
    }

    /// Flush a Lite database before copying/activating it. Never used on legacy input.
    pub fn checkpoint(&self) -> Result<()> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| db("database mutex poisoned"))?;
        let (busy, _, _): (i64, i64, i64) = connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .map_err(db)?;
        if busy != 0 {
            return Err(Error::Conflict(
                "database is busy; checkpoint not complete".into(),
            ));
        }
        Ok(())
    }

    pub fn intake_history(&self, id: &ProjectId) -> Result<Vec<IntakeRevision>> {
        self.transaction(|tx| {
            load_project(tx, id)?;
            let mut statement = tx
                .prepare(
                    "SELECT payload FROM intake_revisions WHERE project_id=?1 ORDER BY revision",
                )
                .map_err(db)?;
            let rows = statement
                .query_map([&id.0], |r| r.get::<_, String>(0))
                .map_err(db)?;
            rows.map(|r| decode(&r.map_err(db)?)).collect()
        })
    }
}

impl Store for SqliteStore {
    fn create_project(&self, project: &Project) -> Result<Project> {
        if project.id.0.is_empty() || project.intake.title.trim().is_empty() {
            return Err(Error::Invalid("project ID and title are required".into()));
        }
        self.transaction(|tx| {
            if let Some(old) = read_json::<Project>(
                tx,
                "SELECT payload FROM projects WHERE id=?1",
                &project.id.0,
            )? {
                return if old == *project {
                    Ok(old)
                } else {
                    Err(Error::Conflict("project ID already exists".into()))
                };
            }
            tx.execute(
                "INSERT INTO projects(id,payload) VALUES(?1,?2)",
                params![project.id.0, encode(project)?],
            )
            .map_err(db)?;
            let revision = IntakeRevision {
                revision: 1,
                intake: project.intake.clone(),
                at_ms: project.created_at_ms,
            };
            tx.execute(
                "INSERT INTO intake_revisions VALUES(?1,1,?2)",
                params![project.id.0, encode(&revision)?],
            )
            .map_err(db)?;
            Ok(project.clone())
        })
    }

    fn list_projects(&self) -> Result<Vec<Project>> {
        self.transaction(|tx| {
            let mut statement = tx
                .prepare("SELECT payload FROM projects ORDER BY id")
                .map_err(db)?;
            let rows = statement
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db)?;
            rows.map(|r| decode(&r.map_err(db)?)).collect()
        })
    }

    fn project(&self, id: &ProjectId) -> Result<Project> {
        self.transaction(|tx| load_project(tx, id))
    }

    fn save_intake(&self, id: &ProjectId, intake: &Intake, at_ms: u64) -> Result<Project> {
        if intake.title.trim().is_empty() {
            return Err(Error::Invalid("title is required".into()));
        }
        self.transaction(|tx| {
            let mut project = load_project(tx, id)?;
            if project.intake == *intake {
                return Ok(project);
            }
            project.intake = intake.clone();
            project.updated_at_ms = at_ms.max(project.updated_at_ms);
            tx.execute(
                "UPDATE projects SET payload=?2 WHERE id=?1",
                params![id.0, encode(&project)?],
            )
            .map_err(db)?;
            let number: i64 = tx
                .query_row(
                    "SELECT coalesce(max(revision),0)+1 FROM intake_revisions WHERE project_id=?1",
                    [&id.0],
                    |r| r.get(0),
                )
                .map_err(db)?;
            let revision = IntakeRevision {
                revision: number as u64,
                intake: intake.clone(),
                at_ms,
            };
            tx.execute(
                "INSERT INTO intake_revisions VALUES(?1,?2,?3)",
                params![id.0, number, encode(&revision)?],
            )
            .map_err(db)?;
            Ok(project)
        })
    }

    fn settings(&self) -> Result<Settings> {
        self.transaction(|tx| {
            let value: Option<String> = tx
                .query_row("SELECT payload FROM settings WHERE id=1", [], |r| r.get(0))
                .optional()
                .map_err(db)?;
            value
                .map(|s| decode(&s))
                .transpose()
                .map(|v| v.unwrap_or_default())
        })
    }

    fn save_settings(&self, settings: &Settings) -> Result<()> {
        self.transaction(|tx| {
            tx.execute("INSERT INTO settings VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", [encode(settings)?]).map_err(db)?;
            Ok(())
        })
    }

    fn create_run(&self, request: &NewRun) -> Result<RunSnapshot> {
        if request.id.0.is_empty() || request.idempotency_key.trim().is_empty() {
            return Err(Error::Invalid(
                "run ID and idempotency key are required".into(),
            ));
        }
        self.transaction(|tx| {
            load_project(tx, &request.project_id)?;
            let old: Option<String> = tx
                .query_row(
                    "SELECT payload FROM runs WHERE project_id=?1 AND idempotency_key=?2",
                    params![request.project_id.0, request.idempotency_key],
                    |r| r.get(0),
                )
                .optional()
                .map_err(db)?;
            if let Some(value) = old {
                let run: RunSnapshot = decode(&value)?;
                return if run.input == request.input && run.rerun_of == request.rerun_of {
                    Ok(run)
                } else {
                    Err(Error::Conflict(
                        "idempotency key reused with different run input".into(),
                    ))
                };
            }
            if let Some(source) = &request.rerun_of
                && load_run(tx, source)?.project_id != request.project_id
            {
                return Err(Error::Invalid(
                    "rerun source belongs to another project".into(),
                ));
            }
            let version: u32 = tx
                .query_row(
                    "SELECT coalesce(max(version),0)+1 FROM runs WHERE project_id=?1",
                    [&request.project_id.0],
                    |r| r.get(0),
                )
                .map_err(db)?;
            let mut run = RunSnapshot {
                id: request.id.clone(),
                project_id: request.project_id.clone(),
                version,
                idempotency_key: request.idempotency_key.clone(),
                input: request.input.clone(),
                rerun_of: request.rerun_of.clone(),
                created_at_ms: request.created_at_ms,
                outcome: RunOutcome::Prepared,
                attempts: vec![],
                checkpoints: vec![],
                last_sequence: 0,
                report: None,
            };
            if read_json::<RunSnapshot>(tx, "SELECT payload FROM runs WHERE id=?1", &run.id.0)?
                .is_some()
            {
                return Err(Error::Conflict("run ID already exists".into()));
            }
            tx.execute(
                "INSERT INTO runs VALUES(?1,?2,?3,?4,?5)",
                params![
                    run.id.0,
                    run.project_id.0,
                    run.version,
                    run.idempotency_key,
                    encode(&run)?
                ],
            )
            .map_err(db)?;
            push_event(
                tx,
                &mut run,
                None,
                EventKind::Prepared,
                request.created_at_ms,
            )?;
            save_run(tx, &run)?;
            Ok(run)
        })
    }

    fn run(&self, id: &RunId) -> Result<RunSnapshot> {
        self.transaction(|tx| load_run(tx, id))
    }

    fn history(&self, project_id: &ProjectId) -> Result<Vec<RunSnapshot>> {
        self.transaction(|tx| {
            load_project(tx, project_id)?;
            let mut statement = tx
                .prepare("SELECT payload FROM runs WHERE project_id=?1 ORDER BY version")
                .map_err(db)?;
            let rows = statement
                .query_map([&project_id.0], |r| r.get::<_, String>(0))
                .map_err(db)?;
            rows.map(|r| decode(&r.map_err(db)?)).collect()
        })
    }

    fn begin_attempt(&self, run_id: &RunId, at_ms: u64) -> Result<Attempt> {
        self.transaction(|tx| {
            let mut run = load_run(tx, run_id)?;
            if run.active_attempt().is_some() || run.outcome == RunOutcome::Completed {
                return Err(Error::Conflict("run is active or completed".into()));
            }
            let number = u32::try_from(run.attempts.len())
                .ok()
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| db("attempt number exhausted"))?;
            let attempt = Attempt {
                id: AttemptId::new(),
                number,
                started_at_ms: at_ms,
                finished_at_ms: None,
                outcome: RunOutcome::Running,
            };
            run.attempts.push(attempt.clone());
            run.outcome = RunOutcome::Running;
            push_event(
                tx,
                &mut run,
                Some(attempt.id.clone()),
                EventKind::AttemptStarted,
                at_ms,
            )?;
            save_run(tx, &run)?;
            Ok(attempt)
        })
    }

    fn append_event(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        event: EventKind,
        at_ms: u64,
    ) -> Result<RunEvent> {
        self.transaction(|tx| {
            let mut run = load_run(tx, run_id)?;
            active(&run, attempt)?;
            if run.outcome != RunOutcome::Running {
                return Err(Error::Conflict("attempt is not running".into()));
            }
            if !matches!(
                event,
                EventKind::StageStarted { .. } | EventKind::TextDelta { .. }
            ) {
                return Err(Error::Invalid(
                    "use the compound operation for state/checkpoint events".into(),
                ));
            }
            let result = push_event(tx, &mut run, Some(attempt.clone()), event, at_ms)?;
            save_run(tx, &run)?;
            Ok(result)
        })
    }

    fn commit_checkpoint(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        checkpoint: &Checkpoint,
    ) -> Result<RunEvent> {
        self.transaction(|tx| {
            let mut run = load_run(tx, run_id)?;
            active(&run, attempt)?;
            if checkpoint.attempt_id != *attempt {
                return Err(Error::Invalid("checkpoint attempt does not match owner".into()));
            }
            if run.outcome != RunOutcome::Running {
                return Err(Error::Conflict("attempt is not running".into()));
            }
            if let Some(existing) = run.checkpoints.iter().find(|c| c.stage == checkpoint.stage) {
                if existing != checkpoint { return Err(Error::Conflict("completed stage is immutable".into())); }
                let mut statement = tx.prepare("SELECT payload FROM events WHERE run_id=?1 ORDER BY sequence").map_err(db)?;
                let rows = statement.query_map([&run_id.0], |r| r.get::<_, String>(0)).map_err(db)?;
                for row in rows {
                    let event: RunEvent = decode(&row.map_err(db)?)?;
                    if matches!(event.event, EventKind::CheckpointSaved { stage, .. } if stage == checkpoint.stage) {
                        return Ok(event);
                    }
                }
                return Err(db("checkpoint has no durable event"));
            }
            if checkpoint.stage.number() != run.checkpoints.len() + 1 {
                return Err(Error::Conflict("checkpoints must commit in stage order".into()));
            }
            run.checkpoints.push(checkpoint.clone());
            let event = push_event(tx, &mut run, Some(attempt.clone()), EventKind::CheckpointSaved {
                stage: checkpoint.stage, usage: checkpoint.usage.clone(),
            }, checkpoint.completed_at_ms)?;
            save_run(tx, &run)?;
            Ok(event)
        })
    }

    fn set_cancelling(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        at_ms: u64,
    ) -> Result<RunSnapshot> {
        self.transaction(|tx| {
            let mut run = load_run(tx, run_id)?;
            active(&run, attempt)?;
            if run.outcome == RunOutcome::Cancelling {
                return Ok(run);
            }
            run.outcome = RunOutcome::Cancelling;
            run.attempts.last_mut().unwrap().outcome = RunOutcome::Cancelling;
            push_event(
                tx,
                &mut run,
                Some(attempt.clone()),
                EventKind::OutcomeChanged {
                    outcome: RunOutcome::Cancelling,
                    message: None,
                },
                at_ms,
            )?;
            save_run(tx, &run)?;
            Ok(run)
        })
    }

    fn finish_attempt(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        outcome: RunOutcome,
        report: Option<&SavedReport>,
        at_ms: u64,
    ) -> Result<RunSnapshot> {
        if !outcome.is_terminal() {
            return Err(Error::Invalid("finish requires a terminal outcome".into()));
        }
        self.transaction(|tx| {
            let mut run = load_run(tx, run_id)?;
            if run.active_attempt().is_none()
                && run.attempts.last().map(|a| &a.id) == Some(attempt)
                && run.outcome == outcome
                && run.report.as_ref() == report
            {
                return Ok(run);
            }
            active(&run, attempt)?;
            if outcome == RunOutcome::Completed {
                if run.outcome == RunOutcome::Cancelling {
                    return Err(Error::Conflict("cancellation already requested".into()));
                }
                let report = report
                    .ok_or_else(|| Error::Invalid("completed run requires a report".into()))?;
                if report.run_id != run.id
                    || report.version != run.version
                    || run.checkpoints.len() != Stage::ALL.len()
                {
                    return Err(Error::Invalid(
                        "report identity/version or completed stages do not match".into(),
                    ));
                }
            } else if report.is_some() {
                return Err(Error::Invalid(
                    "only completed runs may save a report".into(),
                ));
            }
            run.outcome = outcome;
            let owner = run.attempts.last_mut().unwrap();
            owner.outcome = outcome;
            owner.finished_at_ms = Some(at_ms);
            run.report = report.cloned();
            push_event(
                tx,
                &mut run,
                Some(attempt.clone()),
                EventKind::OutcomeChanged {
                    outcome,
                    message: (outcome == RunOutcome::Failed).then(|| {
                        "Run failed. Completed stages were preserved; review settings before resuming.".into()
                    }),
                },
                at_ms,
            )?;
            save_run(tx, &run)?;
            Ok(run)
        })
    }

    fn events_after(&self, cursor: &Subscribe) -> Result<Vec<RunEvent>> {
        self.transaction(|tx| {
            load_run(tx, &cursor.run_id)?;
            if cursor.after_sequence >= i64::MAX as u64 {
                return Ok(vec![]);
            }
            let mut statement = tx
                .prepare(
                    "SELECT payload FROM events WHERE run_id=?1 AND sequence>?2 ORDER BY sequence",
                )
                .map_err(db)?;
            let rows = statement
                .query_map(
                    params![cursor.run_id.0, integer(cursor.after_sequence)?],
                    |r| r.get::<_, String>(0),
                )
                .map_err(db)?;
            rows.map(|r| decode(&r.map_err(db)?)).collect()
        })
    }

    fn recover_interrupted(&self, at_ms: u64) -> Result<Vec<RunId>> {
        self.transaction(|tx| {
            let mut statement = tx
                .prepare("SELECT payload FROM runs ORDER BY id")
                .map_err(db)?;
            let rows = statement
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db)?;
            let runs: Vec<RunSnapshot> = rows
                .map(|r| decode(&r.map_err(db)?))
                .collect::<Result<_>>()?;
            drop(statement);
            let mut recovered = vec![];
            for mut run in runs {
                if let Some(attempt) = run.active_attempt().map(|a| a.id.clone()) {
                    run.outcome = RunOutcome::Interrupted;
                    let owner = run.attempts.last_mut().unwrap();
                    owner.outcome = RunOutcome::Interrupted;
                    owner.finished_at_ms = Some(at_ms);
                    push_event(
                        tx,
                        &mut run,
                        Some(attempt),
                        EventKind::OutcomeChanged {
                            outcome: RunOutcome::Interrupted,
                            message: Some("Recovered after application interruption".into()),
                        },
                        at_ms,
                    )?;
                    save_run(tx, &run)?;
                    recovered.push(run.id);
                }
            }
            Ok(recovered)
        })
    }
}
