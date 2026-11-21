use pfl_desktop::state::{DesktopState, EventApplied, IntakeDraft, Page, RunView};
use pfl_domain::*;

fn project() -> Project {
    Project {
        id: ProjectId("synthetic-project".into()),
        intake: Intake {
            title: "Synthetic project".into(),
            description: "Synthetic only".into(),
            technical_field: "Testing".into(),
            context: InventionContext::default(),
        },
        created_at_ms: 1,
        updated_at_ms: 1,
    }
}

fn snapshot() -> RunSnapshot {
    RunSnapshot {
        id: RunId("synthetic-run".into()),
        project_id: project().id,
        version: 3,
        idempotency_key: "synthetic-key".into(),
        input: RunInput {
            kind: WorkflowKind::Feasibility,
            intake: project().intake,
            provider: ProviderSnapshot {
                kind: ProviderKind::Replay,
                model: "recorded".into(),
                endpoint: None,
            },
            prompts: PromptSnapshot {
                version: "fixture".into(),
                sha256: "fixture".into(),
            },
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
        },
        rerun_of: None,
        created_at_ms: 1,
        outcome: RunOutcome::Running,
        attempts: vec![],
        checkpoints: vec![],
        last_sequence: 0,
        report: None,
    }
}

fn event(sequence: u64, kind: EventKind) -> RunEvent {
    RunEvent {
        run_id: snapshot().id,
        attempt_id: None,
        sequence,
        at_ms: sequence,
        event: kind,
    }
}

#[test]
fn initial_state_and_every_navigation_are_read_only() {
    let mut state = DesktopState::default();
    assert!(state.selected_project.is_none());
    assert!(state.run.is_none());
    state.open(project());
    state.show_run(snapshot());
    for page in [
        Page::Projects,
        Page::Intake,
        Page::History,
        Page::Workflow,
        Page::Report,
    ] {
        state.navigate(page);
        assert_eq!(
            state.selected_run_id(),
            Some(&RunId("synthetic-run".into()))
        );
        assert_eq!(state.run.as_ref().unwrap().sequence, 0);
        assert_eq!(state.run.as_ref().unwrap().snapshot.attempts.len(), 0);
    }
}

#[test]
fn event_replay_is_deduplicated_and_gap_does_not_lose_cursor() {
    let mut view = RunView::new(snapshot());
    let stage = Stage::TechnicalRestatement;
    assert_eq!(
        view.apply(&event(1, EventKind::StageStarted { stage })),
        EventApplied::Applied
    );
    let delta = event(
        2,
        EventKind::TextDelta {
            stage,
            text: "hello".into(),
        },
    );
    assert_eq!(view.apply(&delta), EventApplied::Applied);
    assert_eq!(view.apply(&delta), EventApplied::Duplicate);
    assert_eq!(view.stage_text(stage), "hello");
    assert_eq!(
        view.apply(&event(4, EventKind::Prepared)),
        EventApplied::Gap
    );
    assert_eq!(view.cursor().after_sequence, 2);
    let mut other = event(3, EventKind::Prepared);
    other.run_id = RunId("other".into());
    assert_eq!(view.apply(&other), EventApplied::OtherRun);
    assert_eq!(view.sequence, 2);
    assert_eq!(
        view.apply(&event(3, EventKind::Prepared)),
        EventApplied::Applied
    );
    assert_eq!(
        view.apply(&event(4, EventKind::Prepared)),
        EventApplied::Applied
    );
}

#[test]
fn saved_checkpoints_outrank_streaming_and_historical_events_do_not_regress_status() {
    let mut saved = snapshot();
    saved.last_sequence = 4;
    saved.outcome = RunOutcome::Completed;
    saved.checkpoints.push(Checkpoint {
        stage: Stage::FinalReport,
        attempt_id: AttemptId("attempt".into()),
        text: "durable".into(),
        usage: Usage::default(),
        completed_at_ms: 4,
    });
    let mut view = RunView::new(saved);
    view.apply(&event(
        1,
        EventKind::OutcomeChanged {
            outcome: RunOutcome::Running,
            message: None,
        },
    ));
    view.apply(&event(
        2,
        EventKind::TextDelta {
            stage: Stage::FinalReport,
            text: "transient".into(),
        },
    ));
    assert_eq!(view.snapshot.outcome, RunOutcome::Completed);
    assert_eq!(view.stage_text(Stage::FinalReport), "durable");
    assert!(!view.can_resume());
    assert!(!view.can_cancel());
}

#[test]
fn refresh_does_not_reset_cursor_or_change_selected_historical_run() {
    let mut state = DesktopState::default();
    state.open(project());
    state.show_run(snapshot());
    state
        .run
        .as_mut()
        .unwrap()
        .apply(&event(1, EventKind::Prepared));
    let mut refreshed = snapshot();
    refreshed.last_sequence = 1;
    refreshed.outcome = RunOutcome::Interrupted;
    state.show_run(refreshed);
    assert_eq!(state.run.as_ref().unwrap().cursor().after_sequence, 1);
    assert!(state.run.as_ref().unwrap().can_resume());
    let mut stale = snapshot();
    stale.outcome = RunOutcome::Running;
    state.show_run(stale);
    assert_eq!(
        state.run.as_ref().unwrap().snapshot.outcome,
        RunOutcome::Interrupted
    );
}

#[test]
fn new_project_cannot_inherit_another_projects_report_or_subscription() {
    let mut state = DesktopState::default();
    state.open(project());
    state.show_run(snapshot());
    let mut next = project();
    next.id = ProjectId("next".into());
    state.open(next);
    assert!(state.run.is_none());
    assert!(state.history.is_empty());
    state.show_run(snapshot());
    assert!(state.run.is_none());
}

#[test]
fn draft_keeps_unsaved_edits_until_acknowledged_by_storage() {
    let mut draft = IntakeDraft::new(project().intake);
    assert!(!draft.dirty());
    draft.description.select_all();
    draft
        .description
        .replace_selection("User edited synthetic invention");
    assert!(draft.dirty());
    let persisted = draft.intake();
    assert_eq!(persisted.description, "User edited synthetic invention");
    draft.mark_saved(persisted);
    assert!(!draft.dirty());
}

#[test]
fn basic_edit_preserves_all_nine_optional_fields_and_each_is_editable() {
    let mut intake = project().intake;
    intake.context = pfl_desktop::state::context_from_values(std::array::from_fn(|i| {
        format!("Original optional field {i} café")
    }));
    let original_context = intake.context.clone();
    let mut draft = IntakeDraft::new(intake);
    draft.title.select_all();
    draft.title.replace_selection("Updated title only");
    assert_eq!(draft.intake().context, original_context);
    for index in 0..9 {
        draft.context[index].select_all();
        draft.context[index].replace_selection(&format!("Edited field {index} 世界"));
    }
    let edited = draft.intake();
    for (index, (_, value)) in edited.context.labeled_fields().iter().enumerate() {
        assert_eq!(*value, format!("Edited field {index} 世界"));
    }
    assert_eq!(edited.technical_field, "Testing");
}

#[test]
fn background_updates_extend_history_without_changing_selected_historical_report() {
    let mut state = DesktopState::default();
    state.open(project());
    let mut old = snapshot();
    old.version = 1;
    old.id = RunId("version-one".into());
    old.report = Some(SavedReport {
        run_id: old.id.clone(),
        version: 1,
        markdown: "Older edited report".into(),
    });
    state.show_run(old);
    state.navigate(Page::Report);
    state.remember_run(snapshot());
    assert_eq!(state.selected_run_id(), Some(&RunId("version-one".into())));
    assert_eq!(
        state
            .run
            .as_ref()
            .unwrap()
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .markdown,
        "Older edited report"
    );
    assert_eq!(state.history.len(), 2);
    assert_eq!(state.history[0].version, 3);
}
