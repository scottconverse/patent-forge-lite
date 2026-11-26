//! Automatic's ladder: try one provider, and move to the next only when the failure warrants it.
//!
//! The taxonomy is the whole point. A quota, an unreachable server, a lapsed sign-in, a timeout, an
//! empty answer or an unreadable one are all reasons to try the next rung. A content refusal and an
//! error nobody recognises are not: advancing past a refusal hides a problem the operator needs to
//! see, and advancing past an unknown failure is a guess dressed as resilience.

use crate::registry::{self, Env, ProviderId};
use crate::*;
use std::sync::Arc;

/// Why a call failed, in the terms the ladder reasons about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    /// The provider said the allowance is used up (429, "usage limit", "quota").
    Quota,
    /// Nothing answered, or the server refused to serve (connection refused, 5xx, "unavailable").
    Unavailable,
    /// The sign-in has lapsed or the key was rejected.
    LoginLapse,
    /// No answer inside the budget.
    Timeout,
    /// An answer with no text in it.
    NoOutput,
    /// An answer that could not be read.
    Unreadable,
    /// The model declined. Not a reason to ask another model the same thing.
    ContentRefusal,
    /// Anything else. Also not a reason to move on.
    Unrecognised,
}

impl FailureClass {
    /// Does this failure justify trying the next rung?
    pub fn advances(self) -> bool {
        matches!(
            self,
            FailureClass::Quota
                | FailureClass::Unavailable
                | FailureClass::LoginLapse
                | FailureClass::Timeout
                | FailureClass::NoOutput
                | FailureClass::Unreadable
        )
    }

    pub fn slug(self) -> &'static str {
        match self {
            FailureClass::Quota => "quota",
            FailureClass::Unavailable => "unavailable",
            FailureClass::LoginLapse => "login-lapse",
            FailureClass::Timeout => "timeout",
            FailureClass::NoOutput => "no-output",
            FailureClass::Unreadable => "unreadable",
            FailureClass::ContentRefusal => "content-refusal",
            FailureClass::Unrecognised => "unrecognised",
        }
    }
}

/// Classifies an error by what it says. The messages are this application's own, so this is reading
/// our own words rather than guessing at a vendor's.
pub fn classify(error: &Error) -> FailureClass {
    let text = error.to_string().to_lowercase();
    if matches!(error, Error::Cancelled) {
        // A cancellation is the operator's decision; it never advances the ladder.
        return FailureClass::Unrecognised;
    }
    let has = |needles: &[&str]| needles.iter().any(|needle| text.contains(needle));
    if has(&["rate-limit", "rate limit", "quota", "usage limit", "429"]) {
        FailureClass::Quota
    } else if has(&[
        "key was rejected",
        "401",
        "403",
        "sign in",
        "signed in",
        "logged out",
        "auth",
    ]) {
        FailureClass::LoginLapse
    } else if has(&["did not answer", "timed out", "timeout"]) {
        FailureClass::Timeout
    } else if has(&["returned no text", "no text", "empty answer"]) {
        FailureClass::NoOutput
    } else if has(&[
        "malformed",
        "could not be read",
        "no event stream",
        "no result envelope",
        "no json",
    ]) {
        FailureClass::Unreadable
    } else if has(&["refus", "content policy", "safety", "declined"]) {
        FailureClass::ContentRefusal
    } else if has(&[
        "could not be reached",
        "not running",
        "connection refused",
        "unavailable",
        "500",
        "502",
        "503",
        "504",
        "could not run",
        "no such file",
        "program not found",
    ]) {
        FailureClass::Unavailable
    } else {
        FailureClass::Unrecognised
    }
}

/// One rung that was not used, and why.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkippedRung {
    pub rung: String,
    pub reason: String,
}

/// What a run actually did, in the terms a reader needs.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RunReceipt {
    /// The provider asked for, when one was named.
    pub requested: Option<String>,
    /// The provider that answered.
    pub actual: Option<String>,
    pub effort: Option<String>,
    /// Rungs passed over, in order, with the reason.
    pub skipped: Vec<SkippedRung>,
}

/// The order Automatic will try, given which providers this machine can reach.
pub fn planned_order(env: &Env) -> Vec<ProviderId> {
    registry::automatic_ladder()
        .iter()
        .copied()
        .filter(|id| {
            registry::entry(*id)
                .map(|entry| (entry.enabled)(env))
                .unwrap_or(false)
        })
        .collect()
}

/// Runs the ladder.
///
/// `build` answers a provider for a rung, or `None` when it cannot be built on this machine (it is
/// then recorded as skipped, which is more useful than a silent omission).
pub async fn execute(
    order: &[ProviderId],
    requested: Option<ProviderId>,
    effort: Option<String>,
    build: &dyn Fn(ProviderId) -> Option<Arc<dyn Provider>>,
    request: ProviderRequest,
    cancellation: CancellationToken,
    deltas: tokio::sync::mpsc::UnboundedSender<String>,
) -> Result<(ProviderResponse, RunReceipt)> {
    let mut receipt = RunReceipt {
        requested: requested.map(|id| id.slug().to_string()),
        actual: None,
        effort,
        skipped: Vec::new(),
    };
    let mut last: Option<Error> = None;
    for rung in order {
        let Some(provider) = build(*rung) else {
            receipt.skipped.push(SkippedRung {
                rung: rung.slug().to_string(),
                reason: "not available on this machine".into(),
            });
            continue;
        };
        let mut request = request.clone();
        request.input.provider = provider.snapshot();
        match provider
            .generate(request, cancellation.clone(), deltas.clone())
            .await
        {
            Ok(response) => {
                receipt.actual = Some(rung.slug().to_string());
                return Ok((response, receipt));
            }
            Err(error) => {
                let class = classify(&error);
                if !class.advances() {
                    return Err(error);
                }
                receipt.skipped.push(SkippedRung {
                    rung: rung.slug().to_string(),
                    reason: class.slug().to_string(),
                });
                last = Some(error);
            }
        }
    }
    Err(last.unwrap_or_else(|| {
        Error::Invalid("no provider on the ladder could be built on this machine".into())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::LocalServer;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn error(message: &str) -> Error {
        Error::Provider(message.into())
    }

    #[test]
    fn the_taxonomy_advances_only_where_it_should() {
        let cases = [
            (
                error("the server is rate-limiting; wait and try again"),
                FailureClass::Quota,
                true,
            ),
            (
                error("the model server could not be reached; no alternate destination attempted"),
                FailureClass::Unavailable,
                true,
            ),
            (
                error("the key was rejected; check the key for this server"),
                FailureClass::LoginLapse,
                true,
            ),
            (
                error("the Claude program did not answer within 300s; nothing was committed"),
                FailureClass::Timeout,
                true,
            ),
            (
                error("the Claude CLI returned no text; nothing was committed"),
                FailureClass::NoOutput,
                true,
            ),
            (
                error("the Gemini CLI printed no JSON; saw: boom"),
                FailureClass::Unreadable,
                true,
            ),
            (
                Error::Incomplete("malformed frame in the model stream".into()),
                FailureClass::Unreadable,
                true,
            ),
            (
                error("the model declined: content policy"),
                FailureClass::ContentRefusal,
                false,
            ),
            (
                error("something nobody has seen before"),
                FailureClass::Unrecognised,
                false,
            ),
            (Error::Cancelled, FailureClass::Unrecognised, false),
        ];
        for (error, expected, advances) in cases {
            let class = classify(&error);
            assert_eq!(class, expected, "for {error}");
            assert_eq!(class.advances(), advances, "for {error}");
        }
    }

    struct Fake {
        fail: Option<Error>,
        calls: Arc<AtomicUsize>,
        label: &'static str,
    }

    #[async_trait]
    impl Provider for Fake {
        fn snapshot(&self) -> ProviderSnapshot {
            ProviderSnapshot {
                kind: ProviderKind::Replay,
                model: self.label.into(),
                endpoint: None,
            }
        }
        async fn generate(
            &self,
            _request: ProviderRequest,
            _cancellation: CancellationToken,
            _deltas: tokio::sync::mpsc::UnboundedSender<String>,
        ) -> Result<ProviderResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match &self.fail {
                Some(error) => Err(match error {
                    Error::Provider(message) => Error::Provider(message.clone()),
                    Error::Incomplete(message) => Error::Incomplete(message.clone()),
                    other => Error::Provider(other.to_string()),
                }),
                None => Ok(ProviderResponse {
                    text: "answered".into(),
                    usage: Usage::default(),
                }),
            }
        }
    }

    fn request() -> ProviderRequest {
        let input = RunInput {
            kind: WorkflowKind::Feasibility,
            intake: Intake::default(),
            provider: ProviderSnapshot {
                kind: ProviderKind::Replay,
                model: "rung".into(),
                endpoint: None,
            },
            prompts: PromptSnapshot {
                version: "t".into(),
                sha256: "0".repeat(64),
            },
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
            regeneration: None,
        };
        ProviderRequest {
            run_id: RunId("run".into()),
            attempt_id: AttemptId("attempt".into()),
            stage: Stage::TechnicalRestatement,
            input,
            system_prompt: "s".into(),
            user_prompt: "u".into(),
            prior_outputs: Vec::new(),
            effort: None,
        }
    }

    #[tokio::test]
    async fn a_quota_failure_moves_to_the_next_rung_and_the_receipt_says_so() {
        let calls = Arc::new(AtomicUsize::new(0));

        let build = {
            let calls = calls.clone();
            move |id: ProviderId| -> Option<Arc<dyn Provider>> {
                match id {
                    ProviderId::Claude => Some(Arc::new(Fake {
                        fail: Some(error("rate limit")),
                        calls: calls.clone(),
                        label: "claude",
                    })),
                    ProviderId::Local(LocalServer::LmStudio) => Some(Arc::new(Fake {
                        fail: None,
                        calls: calls.clone(),
                        label: "lmstudio",
                    })),
                    _ => Some(Arc::new(Fake {
                        fail: None,
                        calls: calls.clone(),
                        label: "other",
                    })),
                }
            }
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let (response, receipt) = execute(
            &[ProviderId::Claude, ProviderId::Local(LocalServer::LmStudio)],
            None,
            Some("medium".into()),
            &build,
            request(),
            CancellationToken::new(),
            tx,
        )
        .await
        .unwrap();
        assert_eq!(response.text, "answered");
        assert_eq!(receipt.actual.as_deref(), Some("local-lmstudio"));
        assert_eq!(receipt.skipped.len(), 1);
        assert_eq!(receipt.skipped[0].rung, "claude");
        assert_eq!(receipt.skipped[0].reason, "quota");
        assert_eq!(receipt.effort.as_deref(), Some("medium"));
    }

    #[tokio::test]
    async fn a_refusal_stops_and_is_not_hidden_by_the_next_rung() {
        let calls = Arc::new(AtomicUsize::new(0));
        let build = {
            let calls = calls.clone();
            move |_id: ProviderId| -> Option<Arc<dyn Provider>> {
                Some(Arc::new(Fake {
                    fail: Some(error("the model declined: content policy")),
                    calls: calls.clone(),
                    label: "refusing",
                }))
            }
        };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let error = execute(
            &[ProviderId::Claude, ProviderId::Local(LocalServer::Ollama)],
            None,
            None,
            &build,
            request(),
            CancellationToken::new(),
            tx,
        )
        .await
        .unwrap_err();
        assert!(format!("{error}").contains("declined"));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a refusal must not be retried on another provider"
        );
    }

    #[tokio::test]
    async fn a_rung_that_cannot_be_built_is_recorded_not_skipped_silently() {
        let build = |_id: ProviderId| -> Option<Arc<dyn Provider>> { None };
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let error = execute(
            &[ProviderId::Claude],
            None,
            None,
            &build,
            request(),
            CancellationToken::new(),
            tx,
        )
        .await
        .unwrap_err();
        assert!(format!("{error}").contains("could be built"));
    }

    #[test]
    fn the_planned_order_contains_only_what_this_machine_has() {
        let nothing = Env::default();
        assert!(planned_order(&nothing).is_empty());
        let some = Env {
            has_claude: true,
            ..Env::default()
        };
        assert_eq!(planned_order(&some), vec![ProviderId::Claude]);
    }
}
