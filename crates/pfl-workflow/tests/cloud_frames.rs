//! The Cloud request path, against a loopback stand-in: every way a stream can be wrong.
//!
//! These are the cases the audit found untested — an error frame, a malformed frame, a stream
//! that ends mid-frame, an output-limit stop, and a redirect. Each asserts the specific
//! refusal, so a future change that starts accepting one of them fails here.
use axum::{Router, http::StatusCode, response::IntoResponse, routing::post};
use pfl_domain::*;
use pfl_workflow::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::net::TcpListener;

async fn serve(body: &'static str, status: StatusCode) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = Router::new().route(
            "/v1/messages",
            post(move || async move {
                (
                    status,
                    [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
                    body,
                )
                    .into_response()
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{address}")
}

fn request(snapshot: ProviderSnapshot) -> ProviderRequest {
    let input = RunInput {
        kind: WorkflowKind::Feasibility,
        intake: Intake {
            title: "Frame fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY. A fictional container carries a temperature ",
                "sensor sampled by a controller that reports an unavailable sensor instead of ",
                "inventing a reading. This paragraph exists only to satisfy the word minimum ",
                "and to exercise the Cloud frame path, with café Unicode and no real data.",
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
    }
}

async fn generate(endpoint: &str) -> Result<ProviderResponse> {
    let provider = CloudProvider::with_endpoint("sk-ant-test", "claude-mock", endpoint).unwrap();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_error_frame_is_a_provider_error() {
    let endpoint = serve("data: {\"type\":\"error\"}\n\n", StatusCode::OK).await;
    let error = generate(&endpoint).await.unwrap_err();
    assert!(
        error.to_string().contains("reported an error"),
        "got: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_frame_is_refused_rather_than_skipped() {
    let endpoint = serve("data: {not json}\n\n", StatusCode::OK).await;
    let error = generate(&endpoint).await.unwrap_err();
    assert!(
        error.to_string().contains("malformed frame"),
        "got: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stream_that_ends_before_its_completion_frame_is_incomplete() {
    let body = concat!(
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"partial\"}}\n\n"
    );
    let endpoint = serve(body, StatusCode::OK).await;
    let error = generate(&endpoint).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("ended before its completion frame"),
        "got: {error}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_output_limit_stop_names_the_limit_and_is_incomplete() {
    let body = concat!(
        "data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":7}}}\n\n",
        "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"truncated\"}}\n\n",
        "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"max_tokens\"},\"usage\":{\"output_tokens\":11}}\n\n",
        "data: {\"type\":\"message_stop\"}\n\n"
    );
    let endpoint = serve(body, StatusCode::OK).await;
    let error = generate(&endpoint).await.unwrap_err();
    assert!(error.to_string().contains("output limit"), "got: {error}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_redirect_is_not_followed() {
    // The first host answers with a redirect to the second; the key must not travel there.
    let target_calls = Arc::new(AtomicUsize::new(0));
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target_listener.local_addr().unwrap();
    let counter = target_calls.clone();
    tokio::spawn(async move {
        let app = Router::new().route(
            "/v1/messages",
            post(move || {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, "should not be reached")
                }
            }),
        );
        axum::serve(target_listener, app).await.unwrap();
    });

    let redirect_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_address = redirect_listener.local_addr().unwrap();
    tokio::spawn(async move {
        let location = format!("http://{target_address}/v1/messages");
        let app = Router::new().route(
            "/v1/messages",
            post(move || {
                let location = location.clone();
                async move { (StatusCode::TEMPORARY_REDIRECT, [("location", location)]) }
            }),
        );
        axum::serve(redirect_listener, app).await.unwrap();
    });

    let error = generate(&format!("http://{redirect_address}"))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("HTTP 307"),
        "a redirect must be refused, got: {error}"
    );
    assert_eq!(
        target_calls.load(Ordering::SeqCst),
        0,
        "the key must never reach the redirect target"
    );
}
