//! The OpenAI-compatible request path, against a loopback stand-in.
//!
//! This adapter is what makes LM Studio, llama.cpp, vLLM, Ollama's own `/v1` and every hosted
//! gateway work, so the cases that matter are the ones a real server produces: a normal stream, a
//! usage frame, an error frame, a malformed frame, a stream that stops early, a rejected key, and
//! the header a key actually travels in.
use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use pfl_domain::*;
use pfl_workflow::providers::OpenAiCompatibleProvider;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

#[derive(Clone)]
struct Server {
    body: &'static str,
    status: StatusCode,
    seen: Arc<std::sync::Mutex<Seen>>,
}

#[derive(Default)]
struct Seen {
    authorization: Option<String>,
    request_body: Option<String>,
}

async fn serve(body: &'static str, status: StatusCode) -> (String, Arc<std::sync::Mutex<Seen>>) {
    let seen = Arc::new(std::sync::Mutex::new(Seen::default()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let state = Server {
        body,
        status,
        seen: seen.clone(),
    };
    tokio::spawn(async move {
        let app = Router::new()
            .route(
                "/v1/chat/completions",
                post(
                    |State(server): State<Server>, headers: HeaderMap, body: String| async move {
                        {
                            let mut seen = server.seen.lock().unwrap();
                            seen.authorization = headers
                                .get("authorization")
                                .and_then(|v| v.to_str().ok())
                                .map(str::to_string);
                            seen.request_body = Some(body);
                        }
                        (
                            server.status,
                            [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                            server.body,
                        )
                            .into_response()
                    },
                ),
            )
            .with_state(state);
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), seen)
}

fn request(snapshot: ProviderSnapshot) -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "OpenAI-compatible fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature sensor ",
                "sampled by a controller that reports an unavailable sensor instead of inventing a ",
                "reading. This paragraph exists only to exercise the OpenAI-compatible path.",
            ]
            .concat(),
            technical_field: "temperature sensing containers".into(),
            ..Default::default()
        },
        provider: snapshot,
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
        regeneration: None,        provider_plan: Vec::new(),
    };
    ProviderRequest {
        run_id: RunId("run".into()),
        attempt_id: AttemptId("attempt".into()),
        stage: Stage::TechnicalRestatement,
        input,
        system_prompt: "system".into(),
        user_prompt: "user".into(),
        prior_outputs: Vec::new(),
        effort: None,
    }
}

async fn generate(base: &str, model: &str, key: Option<&str>) -> Result<ProviderResponse> {
    let provider =
        OpenAiCompatibleProvider::new(base.to_string(), model.to_string(), key.map(str::to_string))
            .unwrap();
    let snapshot = provider.snapshot();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    provider
        .generate(
            request(snapshot),
            tokio_util::sync::CancellationToken::new(),
            tx,
        )
        .await
}

const GOOD: &str = concat!(
    "data: {\"choices\":[{\"delta\":{\"content\":\"Paris\"}}]}\n\n",
    "data: {\"choices\":[{\"delta\":{\"content\":\".\"},\"finish_reason\":\"stop\"}]}\n\n",
    "data: {\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":2}}\n\n",
    "data: [DONE]\n\n"
);

#[tokio::test]
async fn a_normal_stream_returns_text_and_usage() {
    let (base, _seen) = serve(GOOD, StatusCode::OK).await;
    let response = generate(&base, "test-model", None).await.unwrap();
    assert_eq!(response.text, "Paris.");
    assert_eq!(response.usage.input_tokens, Some(11));
    assert_eq!(response.usage.output_tokens, Some(2));
}

#[tokio::test]
async fn a_key_travels_in_the_authorization_header_and_a_loopback_call_sends_none() {
    let (base, seen) = serve(GOOD, StatusCode::OK).await;
    generate(&base, "test-model", Some("sk-test-123"))
        .await
        .unwrap();
    assert_eq!(
        seen.lock().unwrap().authorization.as_deref(),
        Some("Bearer sk-test-123")
    );

    let (local, seen_local) = serve(GOOD, StatusCode::OK).await;
    generate(&local, "test-model", None).await.unwrap();
    assert_eq!(
        seen_local.lock().unwrap().authorization,
        None,
        "a local server must not be handed a credential it does not need"
    );
}

#[tokio::test]
async fn an_error_frame_is_a_refusal_not_an_empty_answer() {
    let (base, _seen) = serve(
        "data: {\"error\":{\"message\":\"model not found\"}}\n\n",
        StatusCode::OK,
    )
    .await;
    let error = generate(&base, "missing-model", None).await.unwrap_err();
    assert!(
        format!("{error}").contains("model not found"),
        "got {error}"
    );
}

#[tokio::test]
async fn a_malformed_frame_leaves_nothing_committed() {
    let (base, _seen) = serve("data: {not json}\n\n", StatusCode::OK).await;
    let error = generate(&base, "test-model", None).await.unwrap_err();
    assert!(format!("{error}").contains("malformed"), "got {error}");
}

#[tokio::test]
async fn a_stream_that_stops_before_its_completion_frame_is_incomplete() {
    let (base, _seen) = serve(
        "data: {\"choices\":[{\"delta\":{\"content\":\"half\"}}]}\n\n",
        StatusCode::OK,
    )
    .await;
    let error = generate(&base, "test-model", None).await.unwrap_err();
    assert!(
        format!("{error}").contains("ended before its completion frame"),
        "got {error}"
    );
}

#[tokio::test]
async fn a_rejected_key_says_so_instead_of_reporting_a_generic_failure() {
    let (base, _seen) = serve("", StatusCode::UNAUTHORIZED).await;
    let error = generate(&base, "test-model", Some("sk-wrong"))
        .await
        .unwrap_err();
    let text = format!("{error}");
    assert!(text.contains("401"), "got {text}");
    assert!(text.contains("key was rejected"), "got {text}");
}

#[tokio::test]
async fn a_missing_route_points_at_the_base_url() {
    let (base, _seen) = serve("", StatusCode::NOT_FOUND).await;
    let error = generate(&base, "test-model", None).await.unwrap_err();
    assert!(format!("{error}").contains("base URL"), "got {error}");
}

#[tokio::test]
async fn no_text_at_all_is_an_incomplete_answer() {
    let (base, _seen) = serve(
        "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n",
        StatusCode::OK,
    )
    .await;
    let error = generate(&base, "test-model", None).await.unwrap_err();
    assert!(format!("{error}").contains("no text"), "got {error}");
}

#[tokio::test]
async fn the_request_names_the_model_and_streams_with_usage() {
    let (base, seen) = serve(GOOD, StatusCode::OK).await;
    generate(&base, "qwen-test", None).await.unwrap();
    let body = seen.lock().unwrap().request_body.clone().unwrap();
    assert!(body.contains("\"model\":\"qwen-test\""), "got {body}");
    assert!(body.contains("\"stream\":true"), "got {body}");
    assert!(body.contains("include_usage"), "got {body}");
    assert!(body.contains("\"role\":\"system\""), "got {body}");
    assert!(body.contains("\"role\":\"user\""), "got {body}");
}

#[tokio::test]
async fn a_catalogue_is_read_from_the_models_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = Router::new().route(
            "/v1/models",
            get(|| async {
                (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    r#"{"data":[{"id":"model-a"},{"id":"model-b"}]}"#,
                )
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let models = pfl_workflow::providers::list_openai_models(&format!("http://{address}"))
        .await
        .unwrap();
    assert_eq!(models, vec!["model-a".to_string(), "model-b".to_string()]);
}

#[tokio::test]
async fn cancellation_stops_the_call() {
    let (base, _seen) = serve(GOOD, StatusCode::OK).await;
    let provider =
        Arc::new(OpenAiCompatibleProvider::new(base, "test-model".to_string(), None).unwrap());
    let snapshot = provider.snapshot();
    let cancellation = tokio_util::sync::CancellationToken::new();
    cancellation.cancel();
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        provider.generate(request(snapshot), cancellation, tx),
    )
    .await
    .expect("a cancelled call must return promptly");
    assert!(result.is_err(), "a cancelled call must not report success");
}
