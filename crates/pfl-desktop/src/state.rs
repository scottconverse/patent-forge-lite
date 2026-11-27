//! Read-only presentation projections. No provider or runner lives in a view.
use crate::editor::TextBuffer;
use pfl_domain::{
    EventKind, Intake, InventionContext, Project, RunEvent, RunId, RunOutcome, RunSnapshot, Stage,
    Subscribe,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Projects,
    Intake,
    Workflow,
    History,
    Report,
    Models,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventApplied {
    Applied,
    Duplicate,
    OtherRun,
    Gap,
}

#[derive(Clone, Debug)]
pub struct RunView {
    pub snapshot: RunSnapshot,
    pub sequence: u64,
    pub active_stage: Option<Stage>,
    pub streaming: BTreeMap<Stage, String>,
    pub message: Option<String>,
}

impl RunView {
    pub fn new(snapshot: RunSnapshot) -> Self {
        // Replay from the beginning on initial subscription so in-flight text is
        // reconstructed, not mistaken for a durable checkpoint.
        Self {
            snapshot,
            sequence: 0,
            active_stage: None,
            streaming: BTreeMap::new(),
            message: None,
        }
    }

    pub fn cursor(&self) -> Subscribe {
        Subscribe {
            run_id: self.snapshot.id.clone(),
            after_sequence: self.sequence,
        }
    }

    pub fn apply(&mut self, event: &RunEvent) -> EventApplied {
        if event.run_id != self.snapshot.id {
            return EventApplied::OtherRun;
        }
        if event.sequence <= self.sequence {
            return EventApplied::Duplicate;
        }
        if event.sequence != self.sequence + 1 {
            return EventApplied::Gap;
        }
        self.sequence = event.sequence;
        match &event.event {
            EventKind::Prepared => {}
            EventKind::AttemptStarted => {
                self.streaming.clear();
                self.active_stage = None;
            }
            EventKind::StageStarted { stage } => {
                self.active_stage = Some(*stage);
                self.streaming.insert(*stage, String::new());
            }
            EventKind::TextDelta { stage, text } => {
                self.streaming.entry(*stage).or_default().push_str(text);
            }
            EventKind::CheckpointSaved { .. } => {}
            EventKind::OutcomeChanged { outcome, message } => {
                self.message = message.clone();
                // Historical replay must not regress a newer durable snapshot.
                if event.sequence > self.snapshot.last_sequence {
                    self.snapshot.outcome = *outcome;
                }
            }
        }
        EventApplied::Applied
    }

    pub fn refresh(&mut self, snapshot: RunSnapshot) {
        if snapshot.id == self.snapshot.id && snapshot.last_sequence >= self.snapshot.last_sequence
        {
            self.snapshot = snapshot;
        }
    }

    pub fn stage_text(&self, stage: Stage) -> &str {
        self.snapshot
            .checkpoints
            .iter()
            .find(|c| c.stage == stage)
            .map(|c| c.text.as_str())
            .or_else(|| self.streaming.get(&stage).map(String::as_str))
            .unwrap_or("")
    }

    pub fn can_cancel(&self) -> bool {
        self.snapshot.outcome == RunOutcome::Running
    }
    pub fn can_resume(&self) -> bool {
        matches!(
            self.snapshot.outcome,
            RunOutcome::Prepared
                | RunOutcome::Cancelled
                | RunOutcome::Failed
                | RunOutcome::Interrupted
                | RunOutcome::Truncated
        )
    }
}

#[derive(Clone, Debug)]
pub struct IntakeDraft {
    pub title: TextBuffer,
    pub description: TextBuffer,
    pub technical_field: TextBuffer,
    pub context: [TextBuffer; 9],
    saved: Intake,
}

impl IntakeDraft {
    pub fn new(intake: Intake) -> Self {
        let context = intake
            .context
            .labeled_fields()
            .map(|(_, value)| TextBuffer::new(value, true));
        Self {
            title: TextBuffer::new(intake.title.clone(), false),
            description: TextBuffer::new(intake.description.clone(), true),
            technical_field: TextBuffer::new(intake.technical_field.clone(), false),
            context,
            saved: intake,
        }
    }
    pub fn intake(&self) -> Intake {
        Intake {
            title: self.title.text().to_owned(),
            description: self.description.text().to_owned(),
            technical_field: self.technical_field.text().to_owned(),
            context: context_from_values(std::array::from_fn(|i| {
                self.context[i].text().to_owned()
            })),
        }
    }
    pub fn dirty(&self) -> bool {
        self.intake() != self.saved
    }
    pub fn mark_saved(&mut self, intake: Intake) {
        self.saved = intake;
    }
}

pub fn context_from_values(values: [String; 9]) -> InventionContext {
    let [
        problem_solved,
        how_it_works,
        ai_components,
        three_d_print_components,
        what_is_novel,
        current_alternatives,
        what_is_built,
        what_to_protect,
        additional_notes,
    ] = values;
    InventionContext {
        problem_solved,
        how_it_works,
        ai_components,
        three_d_print_components,
        what_is_novel,
        current_alternatives,
        what_is_built,
        what_to_protect,
        additional_notes,
    }
}

#[derive(Default)]
pub struct DesktopState {
    pub page: Page,
    pub projects: Vec<Project>,
    pub selected_project: Option<Project>,
    pub run: Option<RunView>,
    pub history: Vec<RunSnapshot>,
    pub notice: Option<String>,
}

impl DesktopState {
    pub fn navigate(&mut self, page: Page) {
        self.page = page;
    }

    pub fn open(&mut self, project: Project) {
        let changed = self.selected_project.as_ref().map(|p| &p.id) != Some(&project.id);
        self.selected_project = Some(project);
        if changed {
            self.run = None;
            self.history.clear();
        }
        self.page = Page::Intake;
    }

    pub fn show_run(&mut self, snapshot: RunSnapshot) {
        if self.selected_project.as_ref().map(|p| &p.id) != Some(&snapshot.project_id) {
            return;
        }
        self.remember_run(snapshot.clone());
        if let Some(run) = &mut self.run
            && run.snapshot.id == snapshot.id
        {
            run.refresh(snapshot);
            return;
        }
        self.run = Some(RunView::new(snapshot));
    }

    pub fn remember_run(&mut self, snapshot: RunSnapshot) {
        if self.selected_project.as_ref().map(|p| &p.id) != Some(&snapshot.project_id) {
            return;
        }
        if let Some(saved) = self.history.iter_mut().find(|s| s.id == snapshot.id) {
            if snapshot.last_sequence >= saved.last_sequence {
                *saved = snapshot;
            }
        } else {
            self.history.push(snapshot);
        }
        self.history.sort_by_key(|s| std::cmp::Reverse(s.version));
    }

    pub fn selected_run_id(&self) -> Option<&RunId> {
        self.run.as_ref().map(|r| &r.snapshot.id)
    }
}

/// Everything the start action depends on, as plain values.
///
/// The window decides whether a run may start from several pieces of state in a fixed
/// order. Keeping that decision pure makes it testable without a window, which matters
/// because the order is what protects a paid provider, and because a silent early return
/// here reads to the operator as a dead control.
#[derive(Debug, Clone, Copy, Default)]
pub struct StartGate {
    pub pending: bool,
    pub dirty: bool,
    pub has_project: bool,
    pub run_active: bool,
    pub data_declared: bool,
    /// A mode has been chosen at all. Without one the window used to start compliance on the
    /// canned adapter, because only the plain Start path checked it.
    pub mode_chosen: bool,
    pub mode_is_local: bool,
    pub local_authorized: bool,
    pub mode_is_cloud: bool,
    pub cloud_approved: bool,
}

/// The reason a save cannot be requested, or `None` when it can.
///
/// Pure, like `start_block`, so the in-flight refusal has a test rather than only a code path.
pub fn save_block(pending: bool) -> Option<&'static str> {
    if pending {
        Some("Another request is still in flight; wait for it to finish.")
    } else {
        None
    }
}

/// The reason a run may not start, or `None` when it may. Every refusal carries text, so
/// no path can silently do nothing.
pub fn start_block(gate: &StartGate) -> Option<&'static str> {
    if gate.pending {
        return Some("Another request is still in flight; wait for it to finish.");
    }
    if gate.dirty {
        return Some("Save the current intake before starting a run.");
    }
    if !gate.has_project {
        return Some("Create and save a project first.");
    }
    if gate.run_active {
        return Some("The current run is still active.");
    }
    // One message per missing control: a combined sentence made the operator guess which of
    // three things they had already done.
    if !gate.data_declared {
        return Some("Declare whether this is synthetic test material or a real invention.");
    }
    if !gate.mode_chosen {
        return Some("Choose recorded replay or configure local inference first.");
    }
    if gate.mode_is_local && !gate.local_authorized {
        return Some("Authorize local inference for this endpoint and model.");
    }
    if gate.mode_is_cloud && !gate.cloud_approved {
        return Some("Approve the paid Cloud request for this run.");
    }
    None
}
#[cfg(test)]
mod mode_gate_tests {
    use super::{StartGate, start_block};

    fn ready() -> StartGate {
        StartGate {
            pending: false,
            dirty: false,
            has_project: true,
            run_active: false,
            data_declared: true,
            mode_chosen: true,
            mode_is_local: false,
            local_authorized: false,
            mode_is_cloud: false,
            cloud_approved: false,
        }
    }

    #[test]
    fn a_start_with_no_mode_is_refused_in_the_same_words_as_the_session() {
        let mut gate = ready();
        gate.mode_chosen = false;
        assert_eq!(
            start_block(&gate),
            Some("Choose recorded replay or configure local inference first.")
        );
        gate.mode_chosen = true;
        assert_eq!(start_block(&gate), None);
    }
}

#[cfg(test)]
mod gate_tests {
    use super::{StartGate, start_block};

    fn ready() -> StartGate {
        StartGate {
            pending: false,
            dirty: false,
            has_project: true,
            run_active: false,
            data_declared: true,
            mode_chosen: true,
            mode_is_local: false,
            local_authorized: false,
            mode_is_cloud: false,
            cloud_approved: false,
        }
    }

    #[test]
    fn a_save_while_a_request_is_in_flight_says_so() {
        assert_eq!(super::save_block(false), None);
        assert!(
            super::save_block(true).unwrap().contains("in flight"),
            "the in-flight save refusal must carry text"
        );
    }

    #[test]
    fn a_ready_gate_permits_the_run() {
        assert_eq!(start_block(&ready()), None);
    }

    #[test]
    fn every_refusal_has_text_so_nothing_fails_silently() {
        let mut gate = ready();
        gate.pending = true;
        assert!(start_block(&gate).unwrap().contains("in flight"));
        gate = ready();
        gate.dirty = true;
        assert!(
            start_block(&gate)
                .unwrap()
                .contains("Save the current intake")
        );
        gate = ready();
        gate.has_project = false;
        assert!(start_block(&gate).unwrap().contains("project"));
        gate = ready();
        gate.run_active = true;
        assert!(start_block(&gate).unwrap().contains("still active"));
    }

    #[test]
    fn undeclared_data_blocks_regardless_of_mode() {
        let mut gate = ready();
        gate.data_declared = false;
        assert!(start_block(&gate).unwrap().contains("Declare whether"));
    }

    #[test]
    fn a_local_run_needs_local_authorization_and_a_cloud_run_needs_paid_approval() {
        let mut gate = ready();
        gate.mode_is_local = true;
        assert!(
            start_block(&gate)
                .unwrap()
                .contains("Authorize local inference")
        );
        gate.local_authorized = true;
        assert_eq!(start_block(&gate), None);

        let mut cloud = ready();
        cloud.mode_is_cloud = true;
        assert!(
            start_block(&cloud)
                .unwrap()
                .contains("Approve the paid Cloud request")
        );
        cloud.cloud_approved = true;
        assert_eq!(start_block(&cloud), None);
    }

    #[test]
    fn an_in_flight_request_is_reported_before_anything_else() {
        let mut gate = ready();
        gate.pending = true;
        gate.dirty = true;
        gate.has_project = false;
        assert!(start_block(&gate).unwrap().contains("in flight"));
    }
}
