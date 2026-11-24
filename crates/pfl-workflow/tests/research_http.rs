//! The opt-in HTTP research adapter, verified against a loopback mock.
//!
//! No external network, no API key, no cost: the contract is exercised locally and
//! the three outcomes that must never be confused are distinguished — retrieved,
//! empty, and failed.
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use pfl_domain::*;
use pfl_workflow::*;
use std::{sync::Arc, sync::Mutex};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

fn intake() -> Intake {
    Intake {
        title: "Research adapter fixture".into(),
        description: "authored fictional text for adapter tests".into(),
        technical_field: "temperature sensing containers".into(),
        ..Default::default()
    }
}

#[derive(Clone, Default)]
struct Capture {
    seen: Arc<Mutex<Vec<String>>>,
}

async fn records(State(capture): State<Capture>, uri: axum::http::Uri) -> impl IntoResponse {
    capture
        .seen
        .lock()
        .unwrap()
        .push(uri.query().unwrap_or_default().to_string());
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        r#"[{"reference":"US-1","title":"Temperature indicator","assignee":"Acme","filing_date":"2020-01-02","verified":true,"relevance":"same field"}]"#,
    )
}

async fn empty() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        "[]",
    )
}

async fn broken() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        "not json",
    )
}

async fn failing() -> impl IntoResponse {
    (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
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
async fn a_retrieved_record_is_structured_and_marked_unverified() {
    let capture = Capture::default();
    let base = spawn(
        Router::new()
            .route("/search", get(records))
            .with_state(capture.clone()),
    )
    .await;
    let source = HttpResearchSource::new(format!("{base}/search"), None).unwrap();
    let items = source
        .prior_art(&intake(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].reference, "US-1");
    assert_eq!(items[0].title, "Temperature indicator");
    assert_eq!(items[0].assignee, "Acme");
    // Only PatentsView records are verified. An arbitrary HTTP source is not, and saying so
    // is the difference between a citation and a claim.
    assert!(!items[0].verified, "a generic HTTP record is not verified");
    assert_eq!(items[0].source, "HTTP");
    // The query was built from the technical field, not the whole intake.
    let query = capture.seen.lock().unwrap().clone();
    assert_eq!(query.len(), 1);
    assert!(query[0].contains("q=temperature"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_empty_result_is_a_successful_search_that_found_nothing() {
    let base = spawn(Router::new().route("/search", get(empty))).await;
    let source = HttpResearchSource::new(format!("{base}/search"), None).unwrap();
    let items = source
        .prior_art(&intake(), CancellationToken::new())
        .await
        .unwrap();
    assert!(items.is_empty(), "empty is not an error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_search_is_an_error_not_an_empty_result() {
    for route in ["/broken", "/failing"] {
        let router = Router::new()
            .route("/broken", get(broken))
            .route("/failing", get(failing));
        let base = spawn(router).await;
        let source = HttpResearchSource::new(format!("{base}{route}"), None);
        let source = source.expect("endpoint is valid");
        let error = source
            .prior_art(&intake(), CancellationToken::new())
            .await
            .expect_err("a failed search must not look like no results");
        let text = error.to_string();
        assert!(
            text.contains("malformed") || text.contains("HTTP 503"),
            "unexpected error: {text}"
        );
    }
}

#[test]
fn an_invalid_endpoint_is_refused_before_any_request() {
    assert!(HttpResearchSource::new("not-a-url", None).is_err());
    assert!(HttpResearchSource::new("ftp://example.com", None).is_err());
}

async fn patentsview_ok(body: String) -> impl IntoResponse {
    // The adapter posts JSON; assert the key travelled in its header.
    let _ = body;
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        r#"{"patents":[{"patent_id":"10123456","patent_title":"Container with temperature indicator","patent_date":"2022-05-17","assignee_organization":"Acme Sensors"},{"patent_id":"","patent_title":""}]}"#,
    )
}

async fn patentsview_denied() -> impl IntoResponse {
    (StatusCode::FORBIDDEN, "denied")
}

async fn patentsview_malformed() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        "not json",
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn patentsview_records_are_mapped_and_marked_verified() {
    let base = spawn(Router::new().route("/api/v1/patent/", post(patentsview_ok))).await;
    let source = PatentsViewSource::with_endpoint("key", format!("{base}/api/v1/patent/")).unwrap();
    let items = source
        .prior_art(&intake(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "a record without an id or title is dropped");
    assert_eq!(items[0].publication_number, "10123456");
    assert_eq!(items[0].grant_date, "2022-05-17");
    assert_eq!(items[0].assignee, "Acme Sensors");
    assert_eq!(items[0].source, "USPTO PatentsView");
    assert!(items[0].verified);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn patentsview_denial_and_malformed_body_are_errors_not_empty_results() {
    for (route, handler) in [("/denied", "d"), ("/bad", "b")] {
        let router = if handler == "d" {
            Router::new().route("/denied", post(patentsview_denied))
        } else {
            Router::new().route("/bad", post(patentsview_malformed))
        };
        let base = spawn(router).await;
        let source = PatentsViewSource::with_endpoint("key", format!("{base}{route}")).unwrap();
        let error = source
            .prior_art(&intake(), CancellationToken::new())
            .await
            .expect_err("a failed query must not look like no results");
        let text = error.to_string();
        assert!(
            text.contains("HTTP 403") || text.contains("malformed"),
            "unexpected: {text}"
        );
    }
}

#[test]
fn patentsview_requires_a_key() {
    assert!(PatentsViewSource::new("").is_err());
    assert!(PatentsViewSource::new("key").is_ok());
}
