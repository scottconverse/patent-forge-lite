//! Model readiness: listing an on-machine Ollama's installed models.
//!
//! Loopback only. This is a catalogue read, never inference, and it never starts or
//! installs a runtime.
use axum::{Router, response::IntoResponse, routing::get};
use pfl_workflow::*;
use tokio::net::TcpListener;

async fn tags() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        r#"{"models":[{"name":"phi4-mini:latest"},{"name":"gemma4:e4b"},{"name":"phi4-mini:latest"}]}"#,
    )
}

async fn empty_tags() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        r#"{"models":[]}"#,
    )
}

async fn broken() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        "not json",
    )
}

async fn spawn(router: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{address}")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn installed_models_are_listed_sorted_and_deduplicated() {
    let base = spawn(Router::new().route("/api/tags", get(tags))).await;
    let models = list_local_models(&base).await.unwrap();
    assert_eq!(models, vec!["gemma4:e4b", "phi4-mini:latest"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_empty_catalogue_is_a_successful_read() {
    let base = spawn(Router::new().route("/api/tags", get(empty_tags))).await;
    assert!(list_local_models(&base).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_catalogue_is_an_error() {
    let base = spawn(Router::new().route("/api/tags", get(broken))).await;
    assert!(list_local_models(&base).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_remote_endpoint_is_refused_before_any_request() {
    let error = list_local_models("http://192.0.2.10:11434")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("loopback"));
}
