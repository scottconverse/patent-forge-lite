//! "Regenerate from here onward" through the session actor.
//!
//! The service-level test proves the carried prefix is byte-identical. This proves the
//! window's own request does the same thing, because until now nothing exercised
//! `Request::RegenerateFrom` at all.
use pfl_desktop::session::{ApplicationSession, Mode, Reply, Request};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::ReplayProvider;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn intake() -> Intake {
    Intake {
        title: "Regeneration fixture".into(),
        description: [
            "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature ",
            "sensor sampled by a controller that reports an unavailable sensor instead of ",
            "inventing a reading. This paragraph exists only to satisfy the word minimum ",
            "and to exercise suffix regeneration through the session actor, with café ",
            "Unicode and no real invention data of any kind whatsoever.",
        ]
        .concat(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

fn wait_for(
    session: &ApplicationSession,
    label: &str,
    mut ready: impl FnMut(&Reply) -> bool,
) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        for reply in session.drain() {
            if ready(&reply) {
                return reply;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no matching reply within the deadline at {label}");
}

/// Ask the session for a run until it is terminal, by id, so poll timing cannot matter.
fn run_to_terminal(session: &ApplicationSession, started: RunSnapshot) -> RunSnapshot {
    let id = started.id.clone();
    let mut run = started;
    let deadline = Instant::now() + Duration::from_secs(60);
    while !run.outcome.is_terminal() && Instant::now() < deadline {
        session.request(Request::SelectRun(id.clone())).unwrap();
        match wait_for(session, "select run", |reply| {
            matches!(reply, Reply::Run(_) | Reply::Error(_))
        }) {
            Reply::Run(snapshot) => run = snapshot,
            other => panic!("selecting the run failed: {other:?}"),
        }
        if !run.outcome.is_terminal() {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    run
}

#[test]
fn regenerating_from_a_stage_through_the_session_carries_the_earlier_stages() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let session =
        ApplicationSession::with_replay_provider(store, Arc::new(ReplayProvider::all_workflows()))
            .unwrap();

    session.request(Request::Configure(Mode::Replay)).unwrap();
    match wait_for(&session, "configure", |reply| {
        matches!(reply, Reply::Configured(_) | Reply::Error(_))
    }) {
        Reply::Configured(_) => {}
        other => panic!("configuring replay failed: {other:?}"),
    }

    session.request(Request::Create(intake())).unwrap();
    let project = match wait_for(&session, "create", |reply| {
        matches!(reply, Reply::Opened(_, _) | Reply::Error(_))
    }) {
        Reply::Opened(project, _) => project,
        other => panic!("project creation failed: {other:?}"),
    };

    session
        .request(Request::Start {
            project: project.id.clone(),
            kind: WorkflowKind::Feasibility,
            idempotency_key: "regen-base".into(),
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        })
        .unwrap();
    let base = match wait_for(&session, "base start", |reply| {
        matches!(reply, Reply::Run(_) | Reply::Error(_))
    }) {
        Reply::Run(run) => run_to_terminal(&session, run),
        other => panic!("the base run did not start: {other:?}"),
    };
    assert_eq!(base.outcome, RunOutcome::Completed);
    let stages = base.input.kind.stages();
    let restart = stages[3];

    session
        .request(Request::RegenerateFrom {
            run: base.id.clone(),
            stage: restart,
        })
        .unwrap();
    let regenerated = match wait_for(&session, "regenerate", |reply| {
        matches!(reply, Reply::Run(_) | Reply::Error(_))
    }) {
        Reply::Run(run) => run_to_terminal(&session, run),
        other => panic!("regeneration did not start: {other:?}"),
    };

    assert_eq!(regenerated.outcome, RunOutcome::Completed);
    assert_ne!(regenerated.id, base.id);
    assert_eq!(regenerated.rerun_of.as_ref(), Some(&base.id));
    assert_eq!(regenerated.version, base.version + 1);
    assert_eq!(
        regenerated
            .input
            .regeneration
            .as_ref()
            .map(|plan| plan.from_stage),
        Some(restart)
    );

    // Every stage before the restart point is byte-identical to the base run.
    let carried = |run: &RunSnapshot| -> Vec<(Stage, String)> {
        run.checkpoints
            .iter()
            .filter(|checkpoint| {
                stages
                    .iter()
                    .position(|stage| *stage == checkpoint.stage)
                    .unwrap_or(usize::MAX)
                    < 3
            })
            .map(|checkpoint| (checkpoint.stage, checkpoint.text.clone()))
            .collect()
    };
    assert_eq!(carried(&base), carried(&regenerated));
    assert_eq!(regenerated.checkpoints.len(), base.checkpoints.len());

    // The base run is untouched.
    session
        .request(Request::SelectRun(base.id.clone()))
        .unwrap();
    let reread = match wait_for(&session, "reread base", |reply| {
        matches!(reply, Reply::Run(_) | Reply::Error(_))
    }) {
        Reply::Run(run) => run,
        other => panic!("rereading the base failed: {other:?}"),
    };
    assert_eq!(reread.version, base.version);
    assert_eq!(reread.checkpoints, base.checkpoints);
}
