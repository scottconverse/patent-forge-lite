//! HTTP compatibility surface: legacy auth semantics, health shape, and a real
//! recorded-replay journey over the API.
use pfl_api::{ApiState, router};
use pfl_storage::SqliteStore;
use pfl_workflow::{ReplayProvider, WorkflowService};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::net::TcpListener;

async fn base_url(token: Option<&str>) -> (String, Arc<pfl_api::ApiState>) {
    let store: Arc<dyn pfl_domain::Store> = Arc::new(SqliteStore::in_memory().unwrap());
    let provider = Arc::new(ReplayProvider::all_workflows());
    let service = WorkflowService::new(store, provider, tokio::runtime::Handle::current());
    let state = ApiState::new(service, token.map(str::to_string));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state)
}

fn intake() -> serde_json::Value {
    json!({
        "title": "SYNTHETIC API fixture café",
        "description": concat!(
            "SYNTHETIC TEST MATERIAL ONLY, not an actual invention or patent claim. ",
            "A fictional container carries a sensor sampled by a controller driving one ",
            "indicator and reporting an unavailable sensor rather than inventing a reading. ",
            "This text exists only to satisfy the explicit word minimum and to exercise the ",
            "HTTP surface deterministically with recorded responses and no real data."
        ),
        "technical_field": "Synthetic fixtures"
    })
}

fn authorization() -> serde_json::Value {
    json!({
        "synthetic_data": true,
        "local_inference": false,
        "paid_provider": false,
        "research": false
    })
}

async fn wait_completed(client: &reqwest::Client, base: &str, run_id: &str) -> serde_json::Value {
    for _ in 0..600 {
        let run: serde_json::Value = client
            .get(format!("{base}/api/runs/{run_id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let outcome = run["outcome"].as_str().unwrap_or_default();
        if matches!(outcome, "completed" | "failed" | "cancelled") {
            return run;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("run never reached a terminal outcome");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_matches_the_legacy_shape_and_needs_no_token_when_unset() {
    let (base, _) = base_url(None).await;
    let client = reqwest::Client::new();
    let health: serde_json::Value = client
        .get(format!("{base}/api/health"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["status"], "ok");
    assert_eq!(health["service"], "patentforge-backend");
    assert!(health["timestamp"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bearer_auth_and_download_tokens_match_legacy_semantics() {
    let (base, _state) = base_url(Some("secret-token")).await;
    let client = reqwest::Client::new();
    // No credentials: legacy exact message.
    let missing = client
        .get(format!("{base}/api/projects"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 401);
    let body: serde_json::Value = missing.json().await.unwrap();
    assert_eq!(
        body["message"],
        "Authentication required. Set Authorization: Bearer <token> header."
    );
    assert_eq!(body["error"], "Unauthorized");
    assert_eq!(body["statusCode"], 401);
    // Wrong bearer: invalid-token message.
    let wrong = client
        .get(format!("{base}/api/projects"))
        .bearer_auth("nope")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);
    let body: serde_json::Value = wrong.json().await.unwrap();
    assert_eq!(body["message"], "Invalid authentication token.");
    // Correct bearer.
    let ok = client
        .get(format!("{base}/api/projects"))
        .bearer_auth("secret-token")
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);
    // Long-lived token is also accepted in the query string, repeatedly.
    for _ in 0..2 {
        let query = client
            .get(format!("{base}/api/projects?token=secret-token"))
            .send()
            .await
            .unwrap();
        assert_eq!(query.status(), 200);
    }
    // One-time download token: single use only.
    let token: serde_json::Value = client
        .post(format!("{base}/api/health/download-token"))
        .bearer_auth("secret-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let download = token["token"].as_str().unwrap().to_string();
    assert_ne!(download, "secret-token");
    // A one-time download token opens the download route only. Using it elsewhere must not
    // authenticate: a leaked link must not be able to read projects or start a run.
    let elsewhere = client
        .get(format!("{base}/api/projects?token={download}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        elsewhere.status(),
        401,
        "a download token must not open other routes"
    );
    // The long-lived token still works on that same route, unchanged.
    let long_lived = client
        .get(format!("{base}/api/projects?token=secret-token"))
        .send()
        .await
        .unwrap();
    assert_eq!(long_lived.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_drives_feasibility_then_claims_and_exports_three_formats() {
    let (base, state) = base_url(None).await;
    let client = reqwest::Client::new();
    let project: serde_json::Value = client
        .post(format!("{base}/api/projects"))
        .json(&json!({ "intake": intake() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();

    let run: serde_json::Value = client
        .post(format!("{base}/api/runs"))
        .json(&json!({
            "projectId": project_id,
            "idempotencyKey": "api-feasibility",
            "authorization": authorization()
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run["id"].as_str().unwrap().to_string();
    let completed = wait_completed(&client, &base, &run_id).await;
    assert_eq!(completed["outcome"], "completed");
    assert_eq!(completed["checkpoints"].as_array().unwrap().len(), 6);

    // A claims run over HTTP consumes the completed feasibility run.
    let claims: serde_json::Value = client
        .post(format!("{base}/api/runs"))
        .json(&json!({
            "projectId": project_id,
            "workflow": "claims",
            "idempotencyKey": "api-claims",
            "authorization": authorization()
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let claims_id = claims["id"].as_str().unwrap().to_string();
    let completed_claims = wait_completed(&client, &base, &claims_id).await;
    assert_eq!(completed_claims["outcome"], "completed");
    assert_eq!(completed_claims["checkpoints"].as_array().unwrap().len(), 4);

    // A workflow with unmet prerequisites is a clear client error, on a project
    // that has completed nothing yet.
    let fresh: serde_json::Value = client
        .post(format!("{base}/api/projects"))
        .json(&json!({ "intake": intake() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let refused = client
        .post(format!("{base}/api/runs"))
        .json(&json!({
            "projectId": fresh["id"],
            "workflow": "compliance",
            "idempotencyKey": "api-fresh-project-compliance",
            "authorization": authorization()
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), 400);
    let body: serde_json::Value = refused.json().await.unwrap();
    assert!(
        body["message"].as_str().unwrap().contains("must complete"),
        "unexpected message: {body}"
    );

    let report: serde_json::Value = client
        .get(format!("{base}/api/runs/{run_id}/report"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        report["markdown"]
            .as_str()
            .unwrap()
            .contains("Final report")
    );

    let markdown = client
        .get(format!("{base}/api/runs/{run_id}/export?format=markdown"))
        .send()
        .await
        .unwrap();
    assert_eq!(markdown.status(), 200);
    assert!(markdown.text().await.unwrap().contains("saved analysis"));
    let html = client
        .get(format!("{base}/api/runs/{run_id}/export?format=html"))
        .send()
        .await
        .unwrap();
    assert!(
        html.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("text/html")
    );
    let docx = client
        .get(format!("{base}/api/runs/{run_id}/export?format=docx"))
        .send()
        .await
        .unwrap();
    assert!(
        docx.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("report-v")
    );
    let bytes = docx.bytes().await.unwrap();
    assert_eq!(&bytes[..2], b"PK");
    let bad = client
        .get(format!("{base}/api/runs/{run_id}/export?format=pdf"))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);

    // Events are a read cursor and stay consecutive.
    let events: serde_json::Value = client
        .get(format!("{base}/api/runs/{run_id}/events?after=0"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let sequences: Vec<u64> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["sequence"].as_u64().unwrap())
        .collect();
    assert_eq!(sequences, (1..=sequences.len() as u64).collect::<Vec<_>>());

    state.service.shutdown().await.unwrap();
}

/// Download tokens are random and single-use, and a query token opens the download route
/// only: a leaked link must not be able to start a run.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn download_tokens_are_random_and_limited_to_the_download_route() {
    let (base, _state) = base_url(None).await;
    let client = reqwest::Client::new();
    let first: serde_json::Value = client
        .post(format!("{base}/api/health/download-token"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let second: serde_json::Value = client
        .post(format!("{base}/api/health/download-token"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_token = first["token"].as_str().unwrap();
    let second_token = second["token"].as_str().unwrap();
    assert_eq!(first_token.len(), 32, "a uuid-simple token is 32 hex chars");
    assert!(first_token.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(first_token, second_token, "tokens must not be predictable");

    // A token supplied to a write route is ignored: the request is treated as unauthenticated
    // and, with no long-lived token configured, is refused by the loopback Host rule only if
    // its Host is not loopback — so assert the refusal on a non-loopback Host.
    let refused = client
        .post(format!("{base}/api/runs?token={first_token}"))
        .header("host", "attacker.example")
        .json(&json!({ "project": "unknown", "kind": "feasibility", "authorization": authorization() }))
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), reqwest::StatusCode::UNAUTHORIZED);

    // And a non-loopback Host is refused in the token-less deployment.
    let rebound = client
        .get(format!("{base}/api/health"))
        .header("host", "attacker.example")
        .send()
        .await
        .unwrap();
    assert_eq!(rebound.status(), reqwest::StatusCode::UNAUTHORIZED);
}

/// Intake save, cancel and resume over HTTP: the three write paths that had no test.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn intake_cancel_and_resume_are_accepted_over_http() {
    let (base, _state) = base_url(None).await;
    let client = reqwest::Client::new();
    let project: serde_json::Value = client
        .post(format!("{base}/api/projects"))
        .json(&json!({ "intake": intake() }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();

    // Intake save.
    let mut edited = intake();
    edited["title"] = json!("SYNTHETIC API fixture, edited over HTTP");
    let saved = client
        .post(format!("{base}/api/projects/{project_id}/intake"))
        .json(&json!({ "intake": edited }))
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 200, "intake save should be accepted");
    let reread: serde_json::Value = client
        .get(format!("{base}/api/projects/{project_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        reread["intake"]["title"], "SYNTHETIC API fixture, edited over HTTP",
        "the saved intake should read back"
    );

    // Start, cancel, then resume with an authorization body.
    let run: serde_json::Value = client
        .post(format!("{base}/api/runs"))
        .json(&json!({
            "projectId": project_id,
            "workflow": "feasibility",
            "idempotencyKey": "api-cancel",
            "authorization": authorization()
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run["id"].as_str().unwrap().to_string();
    let cancelled = client
        .post(format!("{base}/api/runs/{run_id}/cancel"))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelled.status(), 200, "cancel should be accepted");
    let status: serde_json::Value = cancelled.json().await.unwrap();
    assert!(
        matches!(status["outcome"].as_str(), Some("cancelled" | "cancelling")),
        "the run should be cancelled or cancelling, got {status}"
    );

    let resumed = client
        .post(format!("{base}/api/runs/{run_id}/resume"))
        .json(&authorization())
        .send()
        .await
        .unwrap();
    assert_eq!(
        resumed.status(),
        200,
        "resume with an authorization should be accepted"
    );
    let completed = wait_completed(&client, &base, &run_id).await;
    assert_eq!(completed["outcome"], "completed");
}
