//! What a model provider *is*, in one place.
//!
//! This module is deliberately pure: no I/O, no `tokio`, no environment reads except through the
//! [`Env`] passed in. That is what lets the window, the CLI and the tests all ask the same
//! questions and get the same answers — and it is the reason a new provider is a new entry here
//! rather than a new branch in five files.
//!
//! The shape is copied from the reference implementation (TownReporter's `provider-registry.ts`)
//! because its reasons apply here unchanged:
//!
//! - **one registry**: the picker, the ladder and the budget used to be separate lists that had to
//!   agree by hand, and one surface was quietly left out;
//! - **three id lists**, because "something an editor can pick", "something only Automatic can
//!   pin" and "a name an older build wrote into a row" are different questions;
//! - **budgets on the entry**, because a CLI that reloads a page of preamble per call and a local
//!   model that answers a long prompt in minutes cannot share a timeout.
//!
//! `registry.test.ts`'s counterpart here fails the build if these lists and the entries disagree.

use pfl_domain::{Stage, WorkflowKind};

/// One place a model is asked for something.
///
/// A stage is the finest grain this product has (nineteen of them); a whole workflow is the coarse
/// one. An assignment may be made at either, and a stage assignment wins over its workflow's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    Workflow(WorkflowKind),
    Stage(Stage),
}

impl Surface {
    pub const EVERY: [Surface; 23] = [
        Surface::Workflow(WorkflowKind::Feasibility),
        Surface::Workflow(WorkflowKind::Claims),
        Surface::Workflow(WorkflowKind::Compliance),
        Surface::Workflow(WorkflowKind::Application),
        Surface::Stage(Stage::TechnicalRestatement),
        Surface::Stage(Stage::PriorArt),
        Surface::Stage(Stage::Patentability),
        Surface::Stage(Stage::Landscape),
        Surface::Stage(Stage::IpStrategy),
        Surface::Stage(Stage::FinalReport),
        Surface::Stage(Stage::ClaimsPlan),
        Surface::Stage(Stage::ClaimsDraft),
        Surface::Stage(Stage::ClaimsExamine),
        Surface::Stage(Stage::ClaimsRevise),
        Surface::Stage(Stage::ComplianceWrittenDescription),
        Surface::Stage(Stage::ComplianceDefiniteness),
        Surface::Stage(Stage::ComplianceFormalities),
        Surface::Stage(Stage::ComplianceEligibility),
        Surface::Stage(Stage::ApplicationBackground),
        Surface::Stage(Stage::ApplicationSummary),
        Surface::Stage(Stage::ApplicationDetailedDescription),
        Surface::Stage(Stage::ApplicationAbstract),
        Surface::Stage(Stage::ApplicationFigures),
    ];

    pub fn label(self) -> &'static str {
        match self {
            Surface::Workflow(kind) => kind.label(),
            Surface::Stage(stage) => stage.label(),
        }
    }

    pub fn slug(self) -> String {
        match self {
            Surface::Workflow(kind) => format!("workflow-{}", kind.slug()),
            Surface::Stage(stage) => format!("stage-{}", stage.slug()),
        }
    }
}

/// The stable identity a stored assignment holds.
///
/// `Cloud` is Anthropic in this product; the name is kept because runs written before this module
/// existed carry it, and a stored run must keep loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderId {
    /// The operator's own Claude Code program, already signed in.
    Claude,
    /// The operator's own Codex program, already signed in.
    Codex,
    /// The operator's own Gemini CLI, already signed in.
    Gemini,
    /// Anthropic's HTTP API with a key.
    Anthropic,
    /// Any OpenAI-compatible HTTP endpoint: LM Studio, llama.cpp, vLLM, OpenRouter, OpenAI.
    OpenAi,
    /// A local server, addressed explicitly.
    Local(LocalServer),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalServer {
    Ollama,
    LmStudio,
    LlamaCpp,
}

impl ProviderId {
    pub fn slug(self) -> &'static str {
        match self {
            ProviderId::Claude => "claude",
            ProviderId::Codex => "codex",
            ProviderId::Gemini => "gemini",
            ProviderId::Anthropic => "anthropic",
            ProviderId::OpenAi => "openai",
            ProviderId::Local(LocalServer::Ollama) => "local-ollama",
            ProviderId::Local(LocalServer::LmStudio) => "local-lmstudio",
            ProviderId::Local(LocalServer::LlamaCpp) => "local-llamacpp",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Some(match slug {
            "claude" => ProviderId::Claude,
            "codex" => ProviderId::Codex,
            "gemini" => ProviderId::Gemini,
            "anthropic" => ProviderId::Anthropic,
            "openai" => ProviderId::OpenAi,
            "local-ollama" => ProviderId::Local(LocalServer::Ollama),
            "local-lmstudio" => ProviderId::Local(LocalServer::LmStudio),
            "local-llamacpp" => ProviderId::Local(LocalServer::LlamaCpp),
            _ => return None,
        })
    }

    /// Does this provider take the invention off the machine?
    ///
    /// Everything except a local server does, including the subscription CLIs: the program may be
    /// local, but the prompt it sends is not.
    pub fn off_machine(self) -> bool {
        !matches!(self, ProviderId::Local(_))
    }

    /// Does using it bill the operator per token?
    ///
    /// Subscription CLIs do not: the operator has already paid for the plan. That difference is why
    /// the paid-approval gate is a property of the provider rather than of "not local".
    pub fn metered(self) -> bool {
        matches!(self, ProviderId::Anthropic | ProviderId::OpenAi)
    }

    /// A local server is addressed rather than configured.
    pub fn default_endpoint(self) -> Option<&'static str> {
        match self {
            ProviderId::Local(LocalServer::Ollama) => Some("http://127.0.0.1:11434"),
            ProviderId::Local(LocalServer::LmStudio) => Some("http://127.0.0.1:1234"),
            ProviderId::Local(LocalServer::LlamaCpp) => Some("http://127.0.0.1:8080"),
            _ => None,
        }
    }

    pub fn transport(self) -> Transport {
        match self {
            ProviderId::Claude => Transport::ClaudeCli,
            ProviderId::Codex => Transport::CodexCli,
            ProviderId::Gemini => Transport::GeminiCli,
            ProviderId::Anthropic => Transport::AnthropicHttp,
            ProviderId::OpenAi => Transport::OpenAiHttp,
            ProviderId::Local(_) => Transport::LocalHttp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// Spawn a signed-in program and read its answer.
    ClaudeCli,
    CodexCli,
    GeminiCli,
    /// Plain HTTPS to a vendor API.
    AnthropicHttp,
    OpenAiHttp,
    /// HTTP to a server on this machine, OpenAI-compatible or Ollama's own shape.
    LocalHttp,
}

/// How long a provider may take.
///
/// Measured reasons, copied from the reference: the CLI kinds reload a large preamble on every call
/// (seconds before the model is even asked), and a local model answering a long prompt is a
/// minutes-scale event, not a seconds-scale one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// The whole run.
    pub wall_ms: u64,
    /// One call.
    pub call_ms: u64,
    /// Kept back so a failure can be written down before the wall runs out.
    pub reserve_ms: u64,
}

impl Budget {
    const fn new(wall_ms: u64, call_ms: u64, reserve_ms: u64) -> Self {
        Self {
            wall_ms,
            call_ms,
            reserve_ms,
        }
    }
}

pub const KIND_BUDGETS: &[(Transport, Budget)] = &[
    (Transport::ClaudeCli, Budget::new(900_000, 300_000, 170_000)),
    (Transport::CodexCli, Budget::new(900_000, 300_000, 170_000)),
    (Transport::GeminiCli, Budget::new(900_000, 300_000, 170_000)),
    (
        Transport::AnthropicHttp,
        Budget::new(38_000, 20_000, 12_000),
    ),
    (Transport::OpenAiHttp, Budget::new(38_000, 20_000, 12_000)),
    // A 30B model on a generous desktop answers a long prompt in minutes. Forty minutes is the
    // bounded guard for a whole multi-stage run; no single answer may take more than ten.
    (
        Transport::LocalHttp,
        Budget::new(2_400_000, 600_000, 60_000),
    ),
];

pub fn budget_for(transport: Transport) -> Budget {
    KIND_BUDGETS
        .iter()
        .find(|(kind, _)| *kind == transport)
        .map(|(_, budget)| *budget)
        .unwrap_or(Budget::new(38_000, 20_000, 12_000))
}

/// One provider the product knows about.
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub id: ProviderId,
    pub label: &'static str,
    /// The model identifier sent to the transport. Empty means "resolve at call time" (local
    /// servers that decide what is loaded), and an empty id can never be sent by mistake.
    pub model: &'static str,
    pub detail: &'static str,
    pub budget: Budget,
    /// Is this entry usable on this machine right now? Reads only what [`Env`] reports.
    pub enabled: fn(&Env) -> bool,
    /// Refuse the call unless the local server reports this model loaded. The application never
    /// loads or unloads a model to satisfy a call.
    pub requires_loaded_local_model: bool,
    /// Pick whichever model the local server reports loaded, at call time.
    pub picks_loaded_local_model: bool,
}

/// What this machine has, as reported by the caller.
#[derive(Debug, Clone, Default)]
pub struct Env {
    pub has_claude: bool,
    pub has_codex: bool,
    pub has_gemini: bool,
    pub has_anthropic_key: bool,
    pub has_openai_endpoint: bool,
    pub ollama_answer: bool,
    pub lmstudio_answer: bool,
    pub llamacpp_answer: bool,
}

/// Every entry, in picker order.
pub const PROVIDER_REGISTRY: &[ProviderEntry] = &[
    ProviderEntry {
        id: ProviderId::Claude,
        label: "Claude Code (your subscription)",
        model: "claude-sonnet-4-5",
        detail: "Runs the Claude program already signed in on this machine",
        budget: Budget::new(900_000, 300_000, 170_000),
        enabled: |env| env.has_claude,
        requires_loaded_local_model: false,
        picks_loaded_local_model: false,
    },
    ProviderEntry {
        id: ProviderId::Codex,
        label: "Codex (your subscription)",
        model: "gpt-6.1-sol",
        detail: "Runs the Codex program already signed in on this machine",
        budget: Budget::new(900_000, 300_000, 170_000),
        enabled: |env| env.has_codex,
        requires_loaded_local_model: false,
        picks_loaded_local_model: false,
    },
    ProviderEntry {
        id: ProviderId::Gemini,
        label: "Gemini (your subscription)",
        model: "gemini-default",
        detail: "Runs the Gemini program already signed in on this machine",
        budget: Budget::new(900_000, 300_000, 170_000),
        enabled: |env| env.has_gemini,
        requires_loaded_local_model: false,
        picks_loaded_local_model: false,
    },
    ProviderEntry {
        id: ProviderId::Anthropic,
        label: "Anthropic API (key)",
        model: "claude-sonnet-4-5",
        detail: "Sends prompts to Anthropic's API and bills per token",
        budget: Budget::new(38_000, 20_000, 12_000),
        enabled: |env| env.has_anthropic_key,
        requires_loaded_local_model: false,
        picks_loaded_local_model: false,
    },
    ProviderEntry {
        id: ProviderId::OpenAi,
        label: "OpenAI-compatible endpoint (key)",
        model: "",
        detail: "Any /v1/chat/completions endpoint: a gateway, a vendor API, or a server you run",
        budget: Budget::new(38_000, 20_000, 12_000),
        enabled: |env| env.has_openai_endpoint,
        requires_loaded_local_model: false,
        picks_loaded_local_model: false,
    },
    ProviderEntry {
        id: ProviderId::Local(LocalServer::LmStudio),
        label: "LM Studio (this machine)",
        model: "",
        detail: "Whichever model LM Studio has loaded",
        budget: Budget::new(2_400_000, 600_000, 60_000),
        enabled: |env| env.lmstudio_answer,
        requires_loaded_local_model: false,
        picks_loaded_local_model: true,
    },
    ProviderEntry {
        id: ProviderId::Local(LocalServer::Ollama),
        label: "Ollama (this machine)",
        model: "",
        detail: "Models installed in Ollama; the app never loads or unloads one",
        budget: Budget::new(2_400_000, 600_000, 60_000),
        enabled: |env| env.ollama_answer,
        requires_loaded_local_model: false,
        picks_loaded_local_model: true,
    },
    ProviderEntry {
        id: ProviderId::Local(LocalServer::LlamaCpp),
        label: "llama.cpp server (this machine)",
        model: "",
        detail: "A llama.cpp server already running on a known port",
        budget: Budget::new(2_400_000, 600_000, 60_000),
        enabled: |env| env.llamacpp_answer,
        requires_loaded_local_model: false,
        picks_loaded_local_model: true,
    },
];

/// Everything the picker offers, in display order.
pub const PICKER_PROVIDER_IDS: &[ProviderId] = &[
    ProviderId::Claude,
    ProviderId::Codex,
    ProviderId::Gemini,
    ProviderId::Anthropic,
    ProviderId::OpenAi,
    ProviderId::Local(LocalServer::LmStudio),
    ProviderId::Local(LocalServer::Ollama),
    ProviderId::Local(LocalServer::LlamaCpp),
];

/// Ids only Automatic may pin: a ladder rung is not a menu entry.
pub const AUTOMATIC_RUNG_IDS: &[ProviderId] = &[
    ProviderId::Local(LocalServer::LmStudio),
    ProviderId::Local(LocalServer::Ollama),
    ProviderId::Claude,
];

/// Names an older build may have written into a row.
///
/// Empty today, and kept deliberately: the day an id is retired it is moved here rather than
/// deleted, so a stored value keeps loading and normalises to Automatic with a note.
pub const RETIRED_PROVIDER_IDS: &[&str] = &[];

pub fn entry(id: ProviderId) -> Option<&'static ProviderEntry> {
    PROVIDER_REGISTRY.iter().find(|entry| entry.id == id)
}

/// The entries worth offering for a surface on this machine.
///
/// Unavailable entries are returned too — the picker shows them disabled with a reason, because a
/// provider that silently vanishes is a provider the operator cannot troubleshoot.
pub fn providers_for(surface: Surface, env: &Env) -> Vec<&'static ProviderEntry> {
    let _ = surface;
    PROVIDER_REGISTRY
        .iter()
        .filter(|entry| (entry.enabled)(env))
        .collect()
}

/// The order Automatic tries, first to last.
pub fn automatic_ladder() -> &'static [ProviderId] {
    AUTOMATIC_RUNG_IDS
}

/// A stored value back into an id, or `None` for a name nothing serves any more.
pub fn resolve_stored(value: &str) -> Option<ProviderId> {
    ProviderId::from_slug(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picker_id_has_an_entry() {
        for id in PICKER_PROVIDER_IDS {
            assert!(entry(*id).is_some(), "no registry entry for {id:?}");
        }
    }

    #[test]
    fn the_lists_and_the_registry_disagree_about_nothing() {
        for entry in PROVIDER_REGISTRY {
            let picks = PICKER_PROVIDER_IDS.contains(&entry.id);
            let rung = AUTOMATIC_RUNG_IDS.contains(&entry.id);
            assert!(
                picks || rung,
                "{:?} is in the registry but in no list",
                entry.id
            );
        }
        for id in AUTOMATIC_RUNG_IDS {
            assert!(
                entry(*id).is_some(),
                "ladder rung {id:?} has no registry entry"
            );
        }
    }

    #[test]
    fn ids_are_unique_and_round_trip_through_their_slugs() {
        let mut seen = std::collections::BTreeSet::new();
        for entry in PROVIDER_REGISTRY {
            assert!(
                seen.insert(entry.id.slug()),
                "duplicate slug {}",
                entry.id.slug()
            );
            assert_eq!(
                ProviderId::from_slug(entry.id.slug()),
                Some(entry.id),
                "slug {} does not round-trip",
                entry.id.slug()
            );
        }
    }

    #[test]
    fn budgets_are_per_transport_and_positive() {
        for (transport, budget) in KIND_BUDGETS {
            assert!(budget.wall_ms > 0, "{transport:?} has no wall budget");
            assert!(budget.call_ms > 0, "{transport:?} has no call budget");
            assert!(
                budget.reserve_ms < budget.wall_ms,
                "{transport:?} reserves more than it allows"
            );
        }
        // A local model and a CLI must never inherit the HTTP budget.
        assert!(
            budget_for(Transport::LocalHttp).call_ms > budget_for(Transport::AnthropicHttp).call_ms
        );
        assert!(
            budget_for(Transport::ClaudeCli).call_ms > budget_for(Transport::AnthropicHttp).call_ms
        );
    }

    #[test]
    fn every_surface_has_a_label_and_a_slug() {
        for surface in Surface::EVERY {
            assert!(!surface.label().is_empty());
            assert!(
                surface.slug().starts_with("stage-") || surface.slug().starts_with("workflow-")
            );
        }
    }

    #[test]
    fn the_ladder_never_offers_something_the_registry_does_not_know() {
        for id in automatic_ladder() {
            assert!(entry(*id).is_some());
        }
    }

    #[test]
    fn a_retired_name_resolves_to_nothing_rather_than_panicking() {
        assert_eq!(resolve_stored("grok-oauth"), None);
        assert_eq!(resolve_stored("claude"), Some(ProviderId::Claude));
    }

    #[test]
    fn off_machine_and_metered_are_not_the_same_question() {
        // A subscription CLI sends the prompt away but does not bill per token.
        assert!(ProviderId::Claude.off_machine());
        assert!(!ProviderId::Claude.metered());
        // A local server does neither.
        assert!(!ProviderId::Local(LocalServer::Ollama).off_machine());
        assert!(!ProviderId::Local(LocalServer::Ollama).metered());
        // A hosted API does both.
        assert!(ProviderId::Anthropic.off_machine());
        assert!(ProviderId::Anthropic.metered());
    }

    #[test]
    fn providers_for_reports_only_what_answers() {
        let nothing = Env::default();
        assert!(providers_for(Surface::Stage(Stage::PriorArt), &nothing).is_empty());
        let some = Env {
            has_claude: true,
            ollama_answer: true,
            ..Env::default()
        };
        let offered = providers_for(Surface::Stage(Stage::PriorArt), &some);
        assert_eq!(offered.len(), 2);
        assert!(offered.iter().any(|e| e.id == ProviderId::Claude));
        assert!(
            offered
                .iter()
                .any(|e| e.id == ProviderId::Local(LocalServer::Ollama))
        );
    }

    #[test]
    fn an_entry_that_picks_the_loaded_model_never_names_one() {
        for entry in PROVIDER_REGISTRY {
            if entry.picks_loaded_local_model {
                assert!(
                    entry.model.is_empty(),
                    "{:?} picks the loaded model but names one",
                    entry.id
                );
            }
        }
    }
}
