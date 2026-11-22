//! Provider wire contracts, verified against local mock servers. No real model
//! or paid provider is ever contacted by these tests.
use axum::{Router, extract::State, http::HeaderMap, response::IntoResponse, routing::post};
use pfl_domain::*;
use pfl_storage::SqliteStore;
use pfl_workflow::*;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

fn request(stage: Stage) -> ProviderRequest {
    ProviderRequest {
        run_id: RunId::new(),
        attempt_id: AttemptId::new(),
        stage,
        input: RunInput {
            kind: stage.kind(),
            intake: Intake {
                title: "Synthetic".into(),
                description: "Synthetic".into(),
                ..Default::default()
            },
            provider: ProviderSnapshot {
                kind: ProviderKind::Replay,
                model: "placeholder".into(),
                endpoint: None,
            },
            prompts: PromptSnapshot {
                version: "v1".into(),
                sha256: "00".into(),
            },
            authorization: Authorization {
                synthetic_data: true,
                ..Default::default()
            },
            upstream: Upstream::default(),
        },
        system_prompt: "system".into(),
        user_prompt: "user".into(),
        prior_outputs: Vec::new(),
    }
}

async fn spawn(app: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[derive(Clone, Default)]
struct CloudCapture {
    seen: Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

async fn cloud_ok(
    State(state): State<CloudCapture>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let mut seen = state.seen.lock().unwrap();
    seen.push((
        headers
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        body,
    ));
    let frames = [
        "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":11}}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"Hello \"}}\n\n",
        "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"café\"}}\n\n",
        "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":7}}\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    ]
    .concat();
    (
        [(axum::http::header::CONTENT_TYPE, "text/event-stream")],
        frames,
    )
}

async fn cloud_fail() -> impl IntoResponse {
    (axum::http::StatusCode::TOO_MANY_REQUESTS, "rate limited")
}

async fn ollama_ok(body: String) -> impl IntoResponse {
    assert!(
        body.contains("\"stream\":true"),
        "Ollama streaming must stay enabled"
    );
    assert!(body.contains("\"model\":\"mock-model\""));
    let frames = [
        "{\"message\":{\"content\":\"Recorded \"},\"done\":false}\n",
        "{\"message\":{\"content\":\"local café\"},\"done\":false}\n",
        "{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\",\"prompt_eval_count\":9,\"eval_count\":5}\n",
    ]
    .concat();
    (
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        frames,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_requires_paid_authorization_before_any_network_call() {
    let capture = CloudCapture::default();
    let base = spawn(
        Router::new()
            .route("/v1/messages", post(cloud_ok))
            .with_state(capture.clone()),
    )
    .await;
    let provider = CloudProvider::with_endpoint("test-key", "mock-model", &base).unwrap();
    let mut req = request(Stage::TechnicalRestatement);
    req.input.provider = provider.snapshot();
    // No paid authorization: refuse locally, with zero requests seen.
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(req.clone(), CancellationToken::new(), tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("paid authorization"));
    assert!(
        capture.seen.lock().unwrap().is_empty(),
        "no call may leave the process"
    );
    // Explicit paid authorization: the documented request shape is used.
    req.input.authorization.paid_provider = true;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let response = provider
        .generate(req, CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(response.text, "Hello café");
    assert_eq!(response.usage.input_tokens, Some(11));
    assert_eq!(response.usage.output_tokens, Some(7));
    let seen = capture.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, "test-key", "the key travels only in the header");
    let body: serde_json::Value = serde_json::from_str(&seen[0].1).unwrap();
    assert_eq!(body["model"], "mock-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["system"], "system");
    assert_eq!(body["messages"][0]["content"], "user");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cloud_http_failure_is_reported_without_a_fallback() {
    let base = spawn(Router::new().route("/v1/messages", post(cloud_fail))).await;
    let provider = CloudProvider::with_endpoint("test-key", "mock-model", &base).unwrap();
    let mut req = request(Stage::TechnicalRestatement);
    req.input.provider = provider.snapshot();
    req.input.authorization.paid_provider = true;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let error = provider
        .generate(req, CancellationToken::new(), tx)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("cloud HTTP 429"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_ollama_wire_format_streams_and_reports_usage() {
    let base = spawn(Router::new().route("/api/chat", post(ollama_ok))).await;
    let provider = OllamaProvider::new(&base, "mock-model").unwrap();
    assert!(provider.is_on_machine(), "a loopback fixture is on-machine");
    let mut req = request(Stage::PriorArt);
    req.input.provider = provider.snapshot();
    req.input.authorization.local_inference = true;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let response = provider
        .generate(req, CancellationToken::new(), tx)
        .await
        .unwrap();
    assert_eq!(response.text, "Recorded local café");
    assert_eq!(response.usage.input_tokens, Some(9));
    assert_eq!(response.usage.output_tokens, Some(5));
    let mut streamed = String::new();
    while let Ok(chunk) = rx.try_recv() {
        streamed.push_str(&chunk);
    }
    assert_eq!(streamed, "Recorded local café");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supplied_research_reaches_the_prior_art_stage_and_is_snapshotted() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let research: Arc<dyn ResearchSource> =
        Arc::new(FixtureResearchSource::new(vec![PriorArtItem {
            reference: "US 7654321 B2".into(),
            relevance: "Synthetic fixture reference for the research seam.".into(),
            ..Default::default()
        }]));
    let service = WorkflowService::new_with_research(
        store.clone(),
        provider.clone(),
        tokio::runtime::Handle::current(),
        Some(research),
    );
    let project = service
        .create_project(Intake {
            title: "Synthetic research fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
                "A fictional container carries a sensor sampled by a controller driving one ",
                "indicator and reporting an unavailable sensor rather than inventing a ",
                "reading, purely so this text satisfies the explicit word minimum and lets ",
                "the prior-art research seam be exercised deterministically with recorded ",
                "responses, no network access and no real invention data whatsoever.",
            ]
            .concat(),
            ..Default::default()
        })
        .await
        .unwrap();
    let run = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "research-1".into(),
            Authorization {
                synthetic_data: true,
                research: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        run.input.upstream.prior_art[0].reference, "US 7654321 B2",
        "research is snapshotted into the run input"
    );
    for _ in 0..600 {
        if service
            .run(run.id.clone())
            .await
            .unwrap()
            .outcome
            .is_terminal()
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let prior_art_request = provider
        .requests()
        .into_iter()
        .find(|request| request.stage == Stage::PriorArt)
        .unwrap();
    assert!(
        prior_art_request.user_prompt.contains("US 7654321 B2"),
        "supplied references must reach the prior-art stage"
    );
    assert!(
        prior_art_request
            .user_prompt
            .contains("Supplied prior-art references")
    );
    // Unauthorized research never queries the source and yields no references.
    let quiet: Arc<dyn ResearchSource> = Arc::new(FixtureResearchSource::new(vec![PriorArtItem {
        reference: "US 9999999 B2".into(),
        relevance: "must not appear".into(),
        ..Default::default()
    }]));
    let other_store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let quiet_service = WorkflowService::new_with_research(
        other_store,
        Arc::new(ReplayProvider::all_workflows()),
        tokio::runtime::Handle::current(),
        Some(quiet),
    );
    let quiet_project = quiet_service
        .create_project(Intake {
            title: "Synthetic no-research fixture".into(),
            description: [
                "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
                "A fictional container carries a sensor sampled by a controller driving one ",
                "indicator and reporting an unavailable sensor rather than inventing a ",
                "reading, purely so this text satisfies the explicit word minimum and lets ",
                "the no-research path be exercised deterministically with recorded ",
                "responses, no network access and no real invention data whatsoever.",
            ]
            .concat(),
            ..Default::default()
        })
        .await
        .unwrap();
    let unauthorized = quiet_service
        .start_workflow(
            quiet_project.id,
            WorkflowKind::Feasibility,
            "research-2".into(),
            Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(unauthorized.input.upstream.prior_art.is_empty());
    let _ = quiet_service.cancel(unauthorized.id).await;
    let _ = service.cancel(run.id).await;
    let _ = service.shutdown().await;
    let _ = quiet_service.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lean_setup_refuses_a_local_runtime_but_full_setup_allows_it() {
    let store: Arc<dyn Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let project = {
        let now = now_ms();
        store
            .create_project(&Project {
                id: ProjectId::new(),
                intake: Intake {
                    title: "Synthetic lean fixture".into(),
                    description: [
                        "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or claim. ",
                        "This text exists only to satisfy the explicit word minimum while the ",
                        "setup-profile rules are exercised deterministically, with no real data, ",
                        "no provider call and no model acquisition of any kind in this test. ",
                        "A fictional cup carries a sensor sampled by a controller that lights ",
                        "one indicator and reports an unavailable sensor rather than inventing ",
                        "a reading, purely so that every required field has ordinary content.",
                    ]
                    .concat(),
                    ..Default::default()
                },
                created_at_ms: now,
                updated_at_ms: now,
            })
            .unwrap()
    };
    store
        .save_settings(&Settings {
            profile: SetupProfile::Lean,
            local_endpoint: None,
            local_model: None,
            research_endpoint: None,
        })
        .unwrap();
    let provider: Arc<dyn Provider> =
        Arc::new(OllamaProvider::new("http://127.0.0.1:1", "m").unwrap());
    let service = WorkflowService::new(store.clone(), provider, tokio::runtime::Handle::current());
    let error = service
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "lean-local".into(),
            Authorization {
                synthetic_data: true,
                local_inference: true,
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("Lean setup is Cloud-only"),
        "{error}"
    );
    // Replay fixtures are always available, and Full setup permits local runtimes.
    let replay = WorkflowService::new(
        store.clone(),
        Arc::new(ReplayProvider::all_workflows()),
        tokio::runtime::Handle::current(),
    );
    let started = replay
        .start_workflow(
            project.id.clone(),
            WorkflowKind::Feasibility,
            "lean-replay".into(),
            Authorization {
                synthetic_data: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(started.input.provider.kind, ProviderKind::Replay);
    // Finish the replay job before starting anything else on this project.
    let _ = replay.cancel(started.id).await;
    store
        .save_settings(&Settings {
            profile: SetupProfile::Full,
            local_endpoint: None,
            local_model: None,
            research_endpoint: None,
        })
        .unwrap();
    let local: Arc<dyn Provider> =
        Arc::new(OllamaProvider::new("http://127.0.0.1:1", "m").unwrap());
    let full = WorkflowService::new(store.clone(), local, tokio::runtime::Handle::current());
    let accepted = full
        .start_workflow(
            project.id,
            WorkflowKind::Feasibility,
            "full-local".into(),
            Authorization {
                synthetic_data: true,
                local_inference: true,
                ..Default::default()
            },
        )
        .await
        .expect("Full setup may use a local runtime");
    assert_eq!(accepted.input.provider.kind, ProviderKind::Local);
    let _ = full.cancel(accepted.id).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), replay.shutdown()).await;
    let _ = tokio::time::timeout(Duration::from_secs(5), full.shutdown()).await;
}
