//! Effort reaching the Anthropic request body (T4).
//!
//! Anthropic takes a token budget rather than a word, and rejects a budget at or above
//! `max_tokens`. These tests pin the mapping, the body it produces, and the refusal of a word
//! nobody defined — before any request is made.
use axum::{Router, extract::State, http::StatusCode, routing::post};
use pfl_domain::*;
use pfl_workflow::*;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

#[derive(Clone)]
struct Captured(Arc<Mutex<Option<String>>>);

async fn serve() -> (String, Arc<Mutex<Option<String>>>) {
    let body = Arc::new(Mutex::new(None));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = Captured(body.clone());
    tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/v1/messages",
                post(|State(captured): State<Captured>, body: String| async move {
                    *captured.0.lock().unwrap() = Some(body);
                    (
                        StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                        concat!(
                            "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":5}}}\n\n",
                            "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"ok\"}}\n\n",
                            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
                            "data: {\"type\":\"message_stop\"}\n\n"
                        ),
                    )
                }),
            )
            .with_state(state);
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), body)
}

fn request(snapshot: &ProviderSnapshot, effort: Option<&str>) -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "Effort fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container reports an unavailable ",
                "sensor rather than inventing a reading. This paragraph exists to exercise the ",
                "Anthropic body builder and carries no real invention.",
            ]
            .concat(),
            technical_field: "temperature sensing".into(),
            ..Default::default()
        },
        provider: snapshot.clone(),
        prompts: PromptSnapshot {
            version: "fixture".into(),
            sha256: "0".repeat(64),
        },
        authorization: Authorization {
            synthetic_data: true,
            paid_provider: true,
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
        system_prompt: "system".into(),
        user_prompt: "user".into(),
        prior_outputs: Vec::new(),
        effort: effort.map(str::to_string),
    }
}

async fn generate(endpoint: &str, effort: Option<&str>) -> Result<ProviderResponse> {
    let provider = CloudProvider::with_endpoint("sk-test", "claude-test", endpoint).unwrap();
    let snapshot = provider.snapshot();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    provider
        .generate(
            request(&snapshot, effort),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
}

#[tokio::test]
async fn an_effort_asks_for_a_thinking_budget_in_the_body() {
    let (base, body) = serve().await;
    generate(&base, Some("medium")).await.unwrap();
    let sent = body.lock().unwrap().clone().unwrap();
    assert!(sent.contains("\"thinking\""), "got {sent}");
    assert!(sent.contains("\"budget_tokens\":4096"), "got {sent}");
    assert!(sent.contains("\"type\":\"enabled\""), "got {sent}");
}

#[tokio::test]
async fn no_effort_sends_no_thinking_field_at_all() {
    let (base, body) = serve().await;
    generate(&base, None).await.unwrap();
    let sent = body.lock().unwrap().clone().unwrap();
    assert!(!sent.contains("thinking"), "got {sent}");
}

#[tokio::test]
async fn high_and_low_map_to_different_budgets() {
    let (base, body) = serve().await;
    generate(&base, Some("high")).await.unwrap();
    let high = body.lock().unwrap().clone().unwrap();
    assert!(high.contains("8192"), "got {high}");

    let (base2, body2) = serve().await;
    generate(&base2, Some("low")).await.unwrap();
    let low = body2.lock().unwrap().clone().unwrap();
    assert!(low.contains("1024"), "got {low}");
}

#[tokio::test]
async fn an_unrecognised_effort_is_refused_before_a_request_is_made() {
    // No server at all: the refusal must happen while building the body.
    let error = generate("http://127.0.0.1:9", Some("extreme"))
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("unknown effort"), "got {text}");
    assert!(text.contains("low, medium or high"), "got {text}");
}
