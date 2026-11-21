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
