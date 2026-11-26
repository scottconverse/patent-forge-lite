//! One view of "what could run this stage", built the same way for the window and the command line.
//!
//! The registry says what *can* exist; discovery says what *is running*; the CLIs say whether they
//! are signed in. This module joins the three so neither surface has to re-implement the join and
//! they cannot disagree about it.

use crate::local_discovery::{LocalCatalog, LocalKind};
use crate::registry::{self, Env, ProviderId};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ModelView {
    pub id: String,
    /// `Some(true)` loaded, `Some(false)` downloaded only, `None` when the server does not say.
    pub loaded: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProviderView {
    pub id: String,
    pub label: String,
    pub detail: String,
    /// Usable on this machine right now.
    pub available: bool,
    /// Would the prompt leave this machine?
    pub off_machine: bool,
    /// Does using it bill per token?
    pub metered: bool,
    /// For a subscription CLI: `Some(true)`/`Some(false)` signed in, `None` not a CLI or unknown.
    pub signed_in: Option<bool>,
    /// For a discovered local server: what it has.
    pub models: Vec<ModelView>,
    /// Why it is not available, in the words an operator needs.
    pub note: Option<String>,
}

fn local_kind(id: ProviderId) -> Option<LocalKind> {
    match id {
        ProviderId::Local(server) => Some(match server {
            registry::LocalServer::Ollama => LocalKind::Ollama,
            registry::LocalServer::LmStudio => LocalKind::LmStudio,
            registry::LocalServer::LlamaCpp => LocalKind::LlamaCpp,
        }),
        _ => None,
    }
}

fn cli_provider(id: ProviderId) -> Option<&'static str> {
    match id {
        ProviderId::Claude => Some("claude"),
        ProviderId::Codex => Some("codex"),
        ProviderId::Gemini => Some("gemini"),
        _ => None,
    }
}

/// Joins the registry, the discovery result and the sign-in states.
///
/// `sign_ins` holds the answer for providers it was actually asked about; anything missing is
/// reported as unknown rather than assumed signed out.
pub fn build(env: &Env, discovered: &LocalCatalog, sign_ins: &[(&str, bool)]) -> Vec<ProviderView> {
    registry::PROVIDER_REGISTRY
        .iter()
        .map(|entry| {
            let id = entry.id;
            let models = local_kind(id)
                .and_then(|kind| {
                    discovered
                        .servers
                        .iter()
                        .find(|server| server.kind == kind)
                        .map(|server| {
                            server
                                .models
                                .iter()
                                .map(|model| ModelView {
                                    id: model.id.clone(),
                                    loaded: model.loaded,
                                })
                                .collect::<Vec<_>>()
                        })
                })
                .unwrap_or_default();
            let signed_in = cli_provider(id).and_then(|name| {
                sign_ins
                    .iter()
                    .find(|(provider, _)| *provider == name)
                    .map(|(_, signed)| *signed)
            });
            let available = (entry.enabled)(env)
                || signed_in == Some(true)
                || (!models.is_empty() && local_kind(id).is_some());
            let note = if available {
                None
            } else if let Some(name) = cli_provider(id) {
                Some(match signed_in {
                    Some(false) => format!("{name} is installed but not signed in"),
                    _ => format!("{name} is not installed"),
                })
            } else if local_kind(id).is_some() {
                Some("nothing answered at this address".to_string())
            } else {
                Some("not configured".to_string())
            };
            ProviderView {
                id: id.slug().to_string(),
                label: entry.label.to_string(),
                detail: entry.detail.to_string(),
                available,
                off_machine: id.off_machine(),
                metered: id.metered(),
                signed_in,
                models,
                note,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_discovery::{LocalModel, LocalServer};

    fn catalog() -> LocalCatalog {
        LocalCatalog {
            servers: vec![LocalServer {
                base_url: "http://127.0.0.1:1234".into(),
                kind: LocalKind::LmStudio,
                models: vec![
                    LocalModel {
                        id: "b".into(),
                        loaded: Some(true),
                        kind: "llm".into(),
                    },
                    LocalModel {
                        id: "a".into(),
                        loaded: Some(false),
                        kind: "llm".into(),
                    },
                ],
            }],
        }
    }

    #[test]
    fn a_discovered_server_lists_its_models_even_when_the_env_disagrees() {
        let env = Env::default();
        let views = build(&env, &catalog(), &[]);
        let lm = views
            .iter()
            .find(|v| v.id == "local-lmstudio")
            .expect("lm studio entry");
        assert!(lm.available, "it answered, so it is available");
        assert_eq!(lm.models.len(), 2);
        assert_eq!(lm.models[0].loaded, Some(true));
        assert_eq!(lm.note, None);
    }

    #[test]
    fn a_cli_reports_its_sign_in_state_and_explains_itself_when_it_cannot_be_used() {
        let env = Env::default();
        let views = build(
            &env,
            &LocalCatalog::default(),
            &[("claude", true), ("codex", false)],
        );
        let claude = views.iter().find(|v| v.id == "claude").unwrap();
        assert!(claude.available);
        assert_eq!(claude.signed_in, Some(true));
        let codex = views.iter().find(|v| v.id == "codex").unwrap();
        assert!(!codex.available);
        assert_eq!(
            codex.note.as_deref(),
            Some("codex is installed but not signed in")
        );
    }

    #[test]
    fn an_unconfigured_http_provider_says_so_instead_of_vanishing() {
        let views = build(&Env::default(), &LocalCatalog::default(), &[]);
        let anthropic = views.iter().find(|v| v.id == "anthropic").unwrap();
        assert!(!anthropic.available);
        assert_eq!(anthropic.note.as_deref(), Some("not configured"));
        assert!(anthropic.off_machine && anthropic.metered);
    }

    #[test]
    fn a_local_provider_with_no_models_is_unavailable_with_a_reason() {
        let views = build(&Env::default(), &LocalCatalog::default(), &[]);
        let ollama = views.iter().find(|v| v.id == "local-ollama").unwrap();
        assert!(!ollama.available);
        assert_eq!(
            ollama.note.as_deref(),
            Some("nothing answered at this address")
        );
    }

    #[test]
    fn every_registry_entry_appears_exactly_once() {
        let views = build(&Env::default(), &LocalCatalog::default(), &[]);
        assert_eq!(views.len(), registry::PROVIDER_REGISTRY.len());
        let mut ids: Vec<_> = views.iter().map(|v| v.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), views.len());
    }
}
