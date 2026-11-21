//! Shared contracts. Storage is synchronous and transactional; async callers use
//! `spawn_blocking` for potentially blocking operations and never await with a DB lock.
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use tokio::sync::mpsc::UnboundedSender;
pub use tokio_util::sync::CancellationToken;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("cancelled")]
    Cancelled,
    #[error("provider: {0}")]
    Provider(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("I/O: {0}")]
    Io(String),
}

macro_rules! id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
id!(ProjectId);
id!(RunId);
id!(AttemptId);

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InventionContext {
    #[serde(alias = "problemSolved")]
    pub problem_solved: String,
    #[serde(alias = "howItWorks")]
    pub how_it_works: String,
    #[serde(alias = "aiComponents")]
    pub ai_components: String,
    #[serde(alias = "threeDPrintComponents")]
    pub three_d_print_components: String,
    #[serde(alias = "whatIsNovel")]
    pub what_is_novel: String,
    #[serde(alias = "currentAlternatives")]
    pub current_alternatives: String,
    #[serde(alias = "whatIsBuilt")]
    pub what_is_built: String,
    #[serde(alias = "whatToProtect")]
    pub what_to_protect: String,
    #[serde(alias = "additionalNotes")]
    pub additional_notes: String,
}

impl InventionContext {
    /// Legacy feasibility input labels and order; callers trim and omit blanks.
    pub fn labeled_fields(&self) -> [(&'static str, &str); 9] {
        [
            ("Problem Solved", &self.problem_solved),
            ("How It Works", &self.how_it_works),
            ("AI / ML Components", &self.ai_components),
            (
                "3D Printing / Physical Design Components",
                &self.three_d_print_components,
            ),
            ("What I Believe Is Novel", &self.what_is_novel),
            (
                "Current Alternatives / Prior Solutions",
                &self.current_alternatives,
            ),
            ("What Has Been Built So Far", &self.what_is_built),
            ("What I Want Protected", &self.what_to_protect),
            ("Additional Notes", &self.additional_notes),
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intake {
    pub title: String,
    pub description: String,
    pub technical_field: String,
    #[serde(default)]
    pub context: InventionContext,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub intake: Intake,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SetupProfile {
    #[default]
    Lean,
    Full,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub profile: SetupProfile,
    pub local_endpoint: Option<String>,
    pub local_model: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    TechnicalRestatement,
    PriorArt,
    Patentability,
    Landscape,
    IpStrategy,
    FinalReport,
}
impl Stage {
    pub const ALL: [Self; 6] = [
        Self::TechnicalRestatement,
        Self::PriorArt,
        Self::Patentability,
        Self::Landscape,
        Self::IpStrategy,
        Self::FinalReport,
    ];
    pub fn number(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap() + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Replay,
    Local,
    Cloud,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSnapshot {
    pub kind: ProviderKind,
    pub model: String,
    /// Destination only, never a URL containing credentials or a secret.
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSnapshot {
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorization {
    pub synthetic_data: bool,
    pub local_inference: bool,
    pub paid_provider: bool,
    pub research: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInput {
    pub intake: Intake,
    pub provider: ProviderSnapshot,
    pub prompts: PromptSnapshot,
    pub authorization: Authorization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewRun {
    pub id: RunId,
    pub project_id: ProjectId,
    /// Unique per project; repeated identical requests return the original run.
    pub idempotency_key: String,
    pub input: RunInput,
    pub rerun_of: Option<RunId>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOutcome {
    Prepared,
    Running,
    Cancelling,
    Cancelled,
    Failed,
    Interrupted,
    Completed,
}
impl RunOutcome {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Failed | Self::Interrupted | Self::Completed
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attempt {
    pub id: AttemptId,
    pub number: u32,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub outcome: RunOutcome,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// None means unknown, not zero.
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub stage: Stage,
    pub attempt_id: AttemptId,
    pub text: String,
    pub usage: Usage,
    pub completed_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedReport {
    pub run_id: RunId,
    pub version: u32,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub id: RunId,
    pub project_id: ProjectId,
    pub version: u32,
    pub idempotency_key: String,
    pub input: RunInput,
    pub rerun_of: Option<RunId>,
    pub created_at_ms: u64,
    pub outcome: RunOutcome,
    pub attempts: Vec<Attempt>,
    pub checkpoints: Vec<Checkpoint>,
    pub last_sequence: u64,
    pub report: Option<SavedReport>,
}
impl RunSnapshot {
    pub fn active_attempt(&self) -> Option<&Attempt> {
        self.attempts.last().filter(|a| a.finished_at_ms.is_none())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum EventKind {
    Prepared,
    AttemptStarted,
    StageStarted {
        stage: Stage,
    },
    TextDelta {
        stage: Stage,
        text: String,
    },
    CheckpointSaved {
        stage: Stage,
        usage: Usage,
    },
    OutcomeChanged {
        outcome: RunOutcome,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEvent {
    pub run_id: RunId,
    pub attempt_id: Option<AttemptId>,
    pub sequence: u64,
    pub at_ms: u64,
    pub event: EventKind,
}

/// Read-only cursor. Subscribe must not start, resume or cancel execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscribe {
    pub run_id: RunId,
    pub after_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Start(NewRun),
    Resume(RunId),
    Rerun { source: RunId, request: NewRun },
    Cancel(RunId),
}

/// All compound writes are atomic. Every attempt-owned mutation rejects stale
/// attempts. A checkpoint, its usage and its event commit together before notification.
pub trait Store: Send + Sync {
    fn create_project(&self, project: &Project) -> Result<Project>;
    fn list_projects(&self) -> Result<Vec<Project>>;
    fn project(&self, id: &ProjectId) -> Result<Project>;
    fn save_intake(&self, id: &ProjectId, intake: &Intake, at_ms: u64) -> Result<Project>;
    fn settings(&self) -> Result<Settings>;
    fn save_settings(&self, settings: &Settings) -> Result<()>;
    fn create_run(&self, request: &NewRun) -> Result<RunSnapshot>;
    fn run(&self, id: &RunId) -> Result<RunSnapshot>;
    fn history(&self, project_id: &ProjectId) -> Result<Vec<RunSnapshot>>;
    fn begin_attempt(&self, run_id: &RunId, at_ms: u64) -> Result<Attempt>;
    fn append_event(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        event: EventKind,
        at_ms: u64,
    ) -> Result<RunEvent>;
    fn commit_checkpoint(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        checkpoint: &Checkpoint,
    ) -> Result<RunEvent>;
    fn set_cancelling(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        at_ms: u64,
    ) -> Result<RunSnapshot>;
    fn finish_attempt(
        &self,
        run_id: &RunId,
        attempt: &AttemptId,
        outcome: RunOutcome,
        report: Option<&SavedReport>,
        at_ms: u64,
    ) -> Result<RunSnapshot>;
    fn events_after(&self, cursor: &Subscribe) -> Result<Vec<RunEvent>>;
    /// Startup-only recovery, never called by a read or a second view.
    fn recover_interrupted(&self, at_ms: u64) -> Result<Vec<RunId>>;
}

#[derive(Debug, Clone)]
pub struct ProviderRequest {
    pub run_id: RunId,
    pub attempt_id: AttemptId,
    pub stage: Stage,
    pub input: RunInput,
    pub system_prompt: String,
    pub user_prompt: String,
    pub prior_outputs: Vec<Checkpoint>,
}

#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub text: String,
    pub usage: Usage,
}

/// Cancellation belongs to one attempt. A provider must never fall back to
/// another destination. Streaming deltas do not imply a durable checkpoint.
#[async_trait]
pub trait Provider: Send + Sync {
    fn snapshot(&self) -> ProviderSnapshot;
    async fn generate(
        &self,
        request: ProviderRequest,
        cancellation: CancellationToken,
        deltas: UnboundedSender<String>,
    ) -> Result<ProviderResponse>;
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_intake_snapshots_default_context_and_partial_context_defaults_fields() {
        let old: Intake = serde_json::from_str(
            r#"{"title":"old","description":"saved","technical_field":"sensors"}"#,
        )
        .unwrap();
        assert_eq!(old.context, InventionContext::default());
        let partial: Intake = serde_json::from_str(
            r#"{"title":"new","description":"saved","technical_field":"","context":{"problem_solved":"problem"}}"#,
        ).unwrap();
        assert_eq!(partial.context.problem_solved, "problem");
        assert_eq!(partial.context.how_it_works, "");
    }

    #[test]
    fn all_nine_legacy_aliases_roundtrip_without_losing_context() {
        let context: InventionContext = serde_json::from_value(serde_json::json!({
            "problemSolved":"one", "howItWorks":"two", "aiComponents":"three",
            "threeDPrintComponents":"four", "whatIsNovel":"five",
            "currentAlternatives":"six", "whatIsBuilt":"seven",
            "whatToProtect":"eight", "additionalNotes":"café α"
        }))
        .unwrap();
        assert_eq!(
            context.labeled_fields().map(|(_, value)| value),
            [
                "one", "two", "three", "four", "five", "six", "seven", "eight", "café α"
            ]
        );
        let encoded = serde_json::to_value(&context).unwrap();
        assert_eq!(encoded["three_d_print_components"], "four");
        assert_eq!(encoded["additional_notes"], "café α");
        assert_eq!(
            serde_json::from_value::<InventionContext>(encoded).unwrap(),
            context
        );
    }
    #[test]
    fn stages_have_stable_order_and_ids_roundtrip() {
        assert_eq!(Stage::ALL.map(Stage::number), [1, 2, 3, 4, 5, 6]);
        let id = RunId::new();
        assert_eq!(
            id,
            serde_json::from_str(&serde_json::to_string(&id).unwrap()).unwrap()
        );
    }
    #[test]
    fn usage_unknown_is_not_zero_and_subscribe_is_only_a_cursor() {
        let usage = Usage::default();
        assert_eq!(usage.output_tokens, None);
        let cursor = Subscribe {
            run_id: RunId::new(),
            after_sequence: 42,
        };
        assert_eq!(cursor.after_sequence, 42);
        assert!(!RunOutcome::Running.is_terminal());
        assert!(RunOutcome::Interrupted.is_terminal());
    }
}
