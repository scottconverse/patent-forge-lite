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
    /// The provider stream ended without its terminal frame. The text received
    /// so far is untrustworthy as a completed result.
    #[error("incomplete provider response: {0}")]
    Incomplete(String),
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
id!(RevisionId);
id!(AttemptId);
/// Which document a revision belongs to. Runs stay immutable; edits become new revisions that
/// point at the run they derive from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentKind {
    Claims,
    Application,
}
impl DocumentKind {
    /// The workflow that generates this document.
    pub fn workflow(self) -> WorkflowKind {
        match self {
            Self::Claims => WorkflowKind::Claims,
            Self::Application => WorkflowKind::Application,
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Claims => "claims",
            Self::Application => "application",
        }
    }
}
/// An edited document revision: a numbered copy derived from a completed run of its kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentRevision {
    pub id: RevisionId,
    pub project_id: ProjectId,
    pub kind: DocumentKind,
    /// 1-based, unique per project and kind.
    pub version: u32,
    /// The immutable generation run this revision was edited from, if any.
    #[serde(default)]
    pub base_run_id: Option<RunId>,
    pub text: String,
    #[serde(default)]
    pub note: String,
    pub created_at_ms: u64,
}

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
    /// Opt-in research endpoint. Absent means research is not configured, and an
    /// authorized run refuses rather than silently searching nothing.
    #[serde(default)]
    pub research_endpoint: Option<String>,
    /// False until the operator has chosen a profile at first run. The window asks
    /// before showing the main surface, rather than assuming one.
    #[serde(default)]
    pub profile_chosen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    // Feasibility analysis (the original six, kept byte-compatible with persisted JSON).
    TechnicalRestatement,
    PriorArt,
    Patentability,
    Landscape,
    IpStrategy,
    FinalReport,
    // Claims drafting.
    ClaimsPlan,
    ClaimsDraft,
    ClaimsExamine,
    ClaimsRevise,
    // Compliance review.
    ComplianceWrittenDescription,
    ComplianceDefiniteness,
    ComplianceFormalities,
    ComplianceEligibility,
    // Application drafting.
    ApplicationBackground,
    ApplicationSummary,
    ApplicationDetailedDescription,
    ApplicationAbstract,
    ApplicationFigures,
}
impl Stage {
    /// The feasibility sequence. Retained under its original name so persisted
    /// feasibility databases, fixtures and tests keep their exact meaning.
    pub const ALL: [Self; 6] = [
        Self::TechnicalRestatement,
        Self::PriorArt,
        Self::Patentability,
        Self::Landscape,
        Self::IpStrategy,
        Self::FinalReport,
    ];
    pub const CLAIMS: [Self; 4] = [
        Self::ClaimsPlan,
        Self::ClaimsDraft,
        Self::ClaimsExamine,
        Self::ClaimsRevise,
    ];
    pub const COMPLIANCE: [Self; 4] = [
        Self::ComplianceWrittenDescription,
        Self::ComplianceDefiniteness,
        Self::ComplianceFormalities,
        Self::ComplianceEligibility,
    ];
    pub const APPLICATION: [Self; 5] = [
        Self::ApplicationBackground,
        Self::ApplicationSummary,
        Self::ApplicationDetailedDescription,
        Self::ApplicationAbstract,
        Self::ApplicationFigures,
    ];
    pub const EVERY_STAGE: [Self; 19] = [
        Self::TechnicalRestatement,
        Self::PriorArt,
        Self::Patentability,
        Self::Landscape,
        Self::IpStrategy,
        Self::FinalReport,
        Self::ClaimsPlan,
        Self::ClaimsDraft,
        Self::ClaimsExamine,
        Self::ClaimsRevise,
        Self::ComplianceWrittenDescription,
        Self::ComplianceDefiniteness,
        Self::ComplianceFormalities,
        Self::ComplianceEligibility,
        Self::ApplicationBackground,
        Self::ApplicationSummary,
        Self::ApplicationDetailedDescription,
        Self::ApplicationAbstract,
        Self::ApplicationFigures,
    ];

    pub fn kind(self) -> WorkflowKind {
        match self {
            Self::TechnicalRestatement
            | Self::PriorArt
            | Self::Patentability
            | Self::Landscape
            | Self::IpStrategy
            | Self::FinalReport => WorkflowKind::Feasibility,
            Self::ClaimsPlan | Self::ClaimsDraft | Self::ClaimsExamine | Self::ClaimsRevise => {
                WorkflowKind::Claims
            }
            Self::ComplianceWrittenDescription
            | Self::ComplianceDefiniteness
            | Self::ComplianceFormalities
            | Self::ComplianceEligibility => WorkflowKind::Compliance,
            Self::ApplicationBackground
            | Self::ApplicationSummary
            | Self::ApplicationDetailedDescription
            | Self::ApplicationAbstract
            | Self::ApplicationFigures => WorkflowKind::Application,
        }
    }

    /// One-based position inside this stage's own workflow.
    pub fn number(self) -> usize {
        self.kind()
            .stages()
            .iter()
            .position(|stage| *stage == self)
            .unwrap_or(0)
            + 1
    }

    /// Stable machine key, unique across every workflow.
    pub fn key(self) -> &'static str {
        match self {
            Self::TechnicalRestatement => "technical_restatement",
            Self::PriorArt => "prior_art",
            Self::Patentability => "patentability",
            Self::Landscape => "landscape",
            Self::IpStrategy => "ip_strategy",
            Self::FinalReport => "final_report",
            Self::ClaimsPlan => "claims_plan",
            Self::ClaimsDraft => "claims_draft",
            Self::ClaimsExamine => "claims_examine",
            Self::ClaimsRevise => "claims_revise",
            Self::ComplianceWrittenDescription => "compliance_written_description",
            Self::ComplianceDefiniteness => "compliance_definiteness",
            Self::ComplianceFormalities => "compliance_formalities",
            Self::ComplianceEligibility => "compliance_eligibility",
            Self::ApplicationBackground => "application_background",
            Self::ApplicationSummary => "application_summary",
            Self::ApplicationDetailedDescription => "application_detailed_description",
            Self::ApplicationAbstract => "application_abstract",
            Self::ApplicationFigures => "application_figures",
        }
    }

    /// Human-facing name, matching the legacy stage titles.
    pub fn label(self) -> &'static str {
        match self {
            Self::TechnicalRestatement => "Technical restatement",
            Self::PriorArt => "Prior art",
            Self::Patentability => "Patentability",
            Self::Landscape => "Landscape",
            Self::IpStrategy => "IP strategy",
            Self::FinalReport => "Final report",
            Self::ClaimsPlan => "Claims strategy",
            Self::ClaimsDraft => "Claim drafting",
            Self::ClaimsExamine => "Examiner review",
            Self::ClaimsRevise => "Claim revision",
            Self::ComplianceWrittenDescription => "Written description",
            Self::ComplianceDefiniteness => "Definiteness",
            Self::ComplianceFormalities => "Formalities",
            Self::ComplianceEligibility => "Eligibility",
            Self::ApplicationBackground => "Background",
            Self::ApplicationSummary => "Summary",
            Self::ApplicationDetailedDescription => "Detailed description",
            Self::ApplicationAbstract => "Abstract",
            Self::ApplicationFigures => "Figures",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    #[default]
    Feasibility,
    Claims,
    Compliance,
    Application,
}

impl WorkflowKind {
    pub const ALL: [Self; 4] = [
        Self::Feasibility,
        Self::Claims,
        Self::Compliance,
        Self::Application,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Feasibility => "feasibility",
            Self::Claims => "claims",
            Self::Compliance => "compliance",
            Self::Application => "application",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Feasibility => "Feasibility analysis",
            Self::Claims => "Claims drafting",
            Self::Compliance => "Compliance review",
            Self::Application => "Application drafting",
        }
    }

    pub fn stages(self) -> &'static [Stage] {
        match self {
            Self::Feasibility => &Stage::ALL,
            Self::Claims => &Stage::CLAIMS,
            Self::Compliance => &Stage::COMPLIANCE,
            Self::Application => &Stage::APPLICATION,
        }
    }

    pub fn stage_count(self) -> usize {
        self.stages().len()
    }

    /// A workflow is only startable once the workflows it consumes are complete.
    pub fn requires(self) -> &'static [WorkflowKind] {
        match self {
            Self::Feasibility => &[],
            Self::Claims => &[Self::Feasibility],
            Self::Compliance => &[Self::Feasibility, Self::Claims],
            Self::Application => &[Self::Feasibility, Self::Claims],
        }
    }

    /// Which upstream stages are snapshotted into a new run's input.
    pub fn upstream_stages(self) -> &'static [Stage] {
        match self {
            Self::Feasibility => &[],
            Self::Claims => &[Stage::IpStrategy, Stage::FinalReport],
            Self::Compliance => &[
                Stage::TechnicalRestatement,
                Stage::IpStrategy,
                Stage::FinalReport,
            ],
            Self::Application => &[
                Stage::TechnicalRestatement,
                Stage::IpStrategy,
                Stage::FinalReport,
            ],
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        Self::ALL.into_iter().find(|kind| {
            kind.slug() == normalized || kind.label().to_ascii_lowercase() == normalized
        })
    }
}

/// One prior-art reference carried into claim and application runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriorArtItem {
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub relevance: String,
    /// Human-readable title of the reference, when known.
    #[serde(default)]
    pub title: String,
    /// Where the record came from: a provider name, or "Manual".
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub publication_number: String,
    #[serde(default)]
    pub filing_date: String,
    #[serde(default)]
    pub grant_date: String,
    #[serde(default)]
    pub assignee: String,
    #[serde(default)]
    pub cpc: String,
    #[serde(default)]
    pub url: String,
    /// True only when the record was retrieved from a provider or entered by the
    /// operator; a model assertion never sets this.
    #[serde(default)]
    pub verified: bool,
}

/// Immutable upstream text a dependent workflow reads. Snapshotted at run
/// creation so a resume can never silently consume a newer analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upstream {
    /// The document revisions this run reviewed, when the operator selected them.
    #[serde(default)]
    pub claims_revision_id: Option<RevisionId>,
    #[serde(default)]
    pub specification_revision_id: Option<RevisionId>,
    /// Feasibility checkpoint text, in workflow order.
    #[serde(default)]
    pub stage_texts: Vec<(Stage, String)>,
    #[serde(default)]
    pub claims_text: String,
    #[serde(default)]
    pub specification_text: String,
    #[serde(default)]
    pub prior_art: Vec<PriorArtItem>,
}

impl Upstream {
    pub fn stage(&self, stage: Stage) -> &str {
        self.stage_texts
            .iter()
            .find(|(candidate, _)| *candidate == stage)
            .map(|(_, text)| text.as_str())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Replay,
    /// A server on this machine, addressed explicitly.
    Local,
    /// Anthropic's API. The name is historical: runs written before other vendors existed carry
    /// it, and a stored run must keep loading.
    Cloud,
    /// The operator's own Claude Code program, already signed in.
    ClaudeCode,
    /// The operator's own Codex program, already signed in.
    Codex,
    /// The operator's own Gemini CLI, already signed in.
    GeminiCli,
    /// Any OpenAI-compatible endpoint: a gateway, a vendor API, or a server on this machine.
    OpenAi,
}

impl<'de> Deserialize<'de> for ProviderKind {
    /// Reads a stored kind, and never fails.
    ///
    /// A run written by a newer build, a typo, or a vendor this build does not know must not make
    /// the database unreadable. An unrecognised name opens as `Replay`, the only kind that cannot
    /// contact anything; the caller records a warning rather than silently running a model.
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(match raw.as_str() {
            "local" => ProviderKind::Local,
            "cloud" => ProviderKind::Cloud,
            "claude_code" => ProviderKind::ClaudeCode,
            "codex" => ProviderKind::Codex,
            "gemini_cli" => ProviderKind::GeminiCli,
            "open_ai" => ProviderKind::OpenAi,
            _ => ProviderKind::Replay,
        })
    }
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
    /// The operator acknowledged this run may process a real (non-synthetic)
    /// invention. Recorded with the run, because consent is part of its identity.
    #[serde(default)]
    pub real_data: bool,
    pub local_inference: bool,
    pub paid_provider: bool,
    pub research: bool,
}

/// "Regenerate from here onward": the new run re-runs `from_stage` and everything
/// after it, carrying the earlier stages from `base_run`. Suffix only, by decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegenerationPlan {
    pub base_run: RunId,
    pub from_stage: Stage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunInput {
    #[serde(default)]
    pub kind: WorkflowKind,
    pub intake: Intake,
    pub provider: ProviderSnapshot,
    pub prompts: PromptSnapshot,
    pub authorization: Authorization,
    #[serde(default)]
    pub upstream: Upstream,
    /// Present only on a regeneration run.
    #[serde(default)]
    pub regeneration: Option<RegenerationPlan>,
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
    /// The provider stopped before its terminal frame. Distinct from Failed and
    /// never presented as Completed.
    Truncated,
}
impl RunOutcome {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Cancelled | Self::Failed | Self::Interrupted | Self::Completed | Self::Truncated
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
    /// Resuming spends again, so it carries the authorization for the retry rather than
    /// reusing whatever the original attempt was approved with.
    Resume {
        id: RunId,
        authorization: Authorization,
    },
    Rerun {
        source: RunId,
        request: NewRun,
    },
    Cancel(RunId),
}

/// Which provider runs one place, and in what order.
///
/// `surface` is a stable slug (`stage-prior-art`, `workflow-claims`), never prose: the label can be
/// reworded without orphaning a stored row. Rank 0 is the choice; ranks 1 and 2 are the fallbacks,
/// tried in order when the one before them fails in a way that warrants moving on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelAssignment {
    pub surface: String,
    pub rank: u8,
    pub provider: String,
    /// Empty means "whatever the server has loaded" for a local provider, and "the entry's own
    /// model" for everything else.
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// A connection the operator defined: a base URL and a model, with the key kept elsewhere.
///
/// There is deliberately no key column. A key lives in the session and never reaches storage;
/// `key_ref` names the in-session credential so a run can be attributed without storing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiConnection {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: Option<String>,
    pub key_ref: Option<String>,
}

/// Per-install overrides for the provider layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderSettings {
    /// A gateway the operator configured, used by the `openai` kind when it has no entry base URL.
    pub openai_base_url: Option<String>,
    /// Where Anthropic's API is reached; loopback is allowed for verification only.
    pub anthropic_endpoint: Option<String>,
    /// A local address to probe in addition to the known ports.
    pub local_endpoint_override: Option<String>,
    /// An operator-chosen whole-run wall clock, in milliseconds.
    pub wall_budget_ms: Option<u64>,
}
/// All compound writes are atomic. Every attempt-owned mutation rejects stale
/// attempts. A checkpoint, its usage and its event commit together before notification.
pub trait Store: Send + Sync {
    fn create_project(&self, project: &Project) -> Result<Project>;
    fn list_projects(&self) -> Result<Vec<Project>>;
    fn project(&self, id: &ProjectId) -> Result<Project>;
    fn save_intake(&self, id: &ProjectId, intake: &Intake, at_ms: u64) -> Result<Project>;
    fn save_revision(&self, revision: &DocumentRevision) -> Result<DocumentRevision>;
    fn revisions(&self, project: &ProjectId, kind: DocumentKind) -> Result<Vec<DocumentRevision>>;
    fn revision(&self, id: &RevisionId) -> Result<DocumentRevision>;
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
    /// Every assignment for a project, ordered by surface and rank.
    fn assignments(&self, project: &ProjectId) -> Result<Vec<ModelAssignment>>;
    fn set_assignment(&self, project: &ProjectId, assignment: &ModelAssignment) -> Result<()>;
    /// Removes every rank for one surface, so a cleared choice leaves nothing behind.
    fn clear_assignments(&self, project: &ProjectId, surface: &str) -> Result<usize>;
    fn provider_settings(&self) -> Result<ProviderSettings>;
    fn save_provider_settings(&self, settings: &ProviderSettings) -> Result<()>;
    fn connections(&self) -> Result<Vec<ApiConnection>>;
    fn save_connection(&self, connection: &ApiConnection) -> Result<()>;
    fn delete_connection(&self, id: &str) -> Result<()>;
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
    /// How hard the model should think, when the transport accepts a run-scoped setting.
    ///
    /// This travels with the call rather than with [`ProviderSnapshot`]: effort does not change
    /// which provider a run used, and putting it in the snapshot would make a resumed run refuse
    /// its own provider because someone adjusted how hard it should think.
    pub effort: Option<String>,
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
    fn every_workflow_numbers_its_own_stages_from_one() {
        for kind in WorkflowKind::ALL {
            let stages = kind.stages();
            assert_eq!(stages.len(), kind.stage_count());
            for (index, stage) in stages.iter().enumerate() {
                assert_eq!(stage.kind(), kind);
                assert_eq!(stage.number(), index + 1);
            }
        }
        assert_eq!(Stage::EVERY_STAGE.len(), 19);
        assert_eq!(
            WorkflowKind::ALL.map(WorkflowKind::stage_count),
            [6, 4, 4, 5]
        );
    }

    #[test]
    fn dependent_workflows_declare_the_upstream_they_snapshot() {
        assert!(WorkflowKind::Feasibility.requires().is_empty());
        assert_eq!(WorkflowKind::Claims.requires(), [WorkflowKind::Feasibility]);
        assert_eq!(
            WorkflowKind::Compliance.requires(),
            [WorkflowKind::Feasibility, WorkflowKind::Claims]
        );
        assert_eq!(
            WorkflowKind::Application.requires(),
            [WorkflowKind::Feasibility, WorkflowKind::Claims]
        );
        assert_eq!(
            WorkflowKind::Claims.upstream_stages(),
            [Stage::IpStrategy, Stage::FinalReport]
        );
        assert!(WorkflowKind::Feasibility.upstream_stages().is_empty());
    }

    #[test]
    fn persisted_feasibility_runs_keep_their_original_shape() {
        // A pre-workflow database stores bare stage names and no workflow fields.
        let old: Stage = serde_json::from_str(r#""final_report""#).unwrap();
        assert_eq!(old, Stage::FinalReport);
        assert_eq!(old.kind(), WorkflowKind::Feasibility);
        let input: RunInput = serde_json::from_value(serde_json::json!({
            "intake": {"title":"t","description":"d","technical_field":"f"},
            "provider": {"kind":"replay","model":"m","endpoint":null},
            "prompts": {"version":"v1","sha256":"00"},
            "authorization": {
                "synthetic_data": true, "local_inference": false,
                "paid_provider": false, "research": false
            }
        }))
        .unwrap();
        assert_eq!(input.kind, WorkflowKind::Feasibility);
        assert_eq!(input.upstream, Upstream::default());
        let encoded = serde_json::to_value(&input).unwrap();
        assert_eq!(encoded["kind"], "feasibility");
        assert_eq!(encoded["upstream"]["claims_text"], "");
    }

    #[test]
    fn workflow_kinds_parse_from_slug_and_label() {
        assert_eq!(WorkflowKind::parse("claims"), Some(WorkflowKind::Claims));
        assert_eq!(
            WorkflowKind::parse(" Compliance Review "),
            Some(WorkflowKind::Compliance)
        );
        assert_eq!(WorkflowKind::parse("nonsense"), None);
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

impl Stage {
    /// The stable key a stored assignment uses. Never derived from the label, which is prose.
    pub fn slug(self) -> &'static str {
        match self {
            Self::TechnicalRestatement => "technical-restatement",
            Self::PriorArt => "prior-art",
            Self::Patentability => "patentability",
            Self::Landscape => "landscape",
            Self::IpStrategy => "ip-strategy",
            Self::FinalReport => "final-report",
            Self::ClaimsPlan => "claims-plan",
            Self::ClaimsDraft => "claims-draft",
            Self::ClaimsExamine => "claims-examine",
            Self::ClaimsRevise => "claims-revise",
            Self::ComplianceWrittenDescription => "compliance-written-description",
            Self::ComplianceDefiniteness => "compliance-definiteness",
            Self::ComplianceFormalities => "compliance-formalities",
            Self::ComplianceEligibility => "compliance-eligibility",
            Self::ApplicationBackground => "application-background",
            Self::ApplicationSummary => "application-summary",
            Self::ApplicationDetailedDescription => "application-detailed-description",
            Self::ApplicationAbstract => "application-abstract",
            Self::ApplicationFigures => "application-figures",
        }
    }
}

#[cfg(test)]
mod provider_kind_compat {
    use super::*;

    #[test]
    fn the_kinds_this_build_writes_round_trip() {
        for kind in [
            ProviderKind::Replay,
            ProviderKind::Local,
            ProviderKind::Cloud,
            ProviderKind::ClaudeCode,
            ProviderKind::Codex,
            ProviderKind::GeminiCli,
            ProviderKind::OpenAi,
        ] {
            let text = serde_json::to_string(&kind).unwrap();
            assert_eq!(serde_json::from_str::<ProviderKind>(&text).unwrap(), kind);
        }
    }

    #[test]
    fn a_run_written_before_other_vendors_existed_still_loads() {
        let stored = r#"{"kind":"cloud","model":"claude-sonnet-4-5","endpoint":null}"#;
        let snapshot: ProviderSnapshot = serde_json::from_str(stored).unwrap();
        assert_eq!(snapshot.kind, ProviderKind::Cloud);
        assert_eq!(snapshot.model, "claude-sonnet-4-5");
    }

    #[test]
    fn a_kind_this_build_does_not_know_opens_as_replay_instead_of_failing() {
        let stored = r#"{"kind":"some_future_vendor","model":"x","endpoint":null}"#;
        let snapshot: ProviderSnapshot = serde_json::from_str(stored).unwrap();
        assert_eq!(snapshot.kind, ProviderKind::Replay);
    }
}
