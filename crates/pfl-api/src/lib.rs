//! Compatibility HTTP API for Patent Forge Lite.
//!
//! Auth intentionally mirrors the legacy NestJS guard: when a token is
//! configured every `/api` route requires `Authorization: Bearer <token>`, and a
//! `?token=` query parameter is still accepted for one-time download tokens
//! (single use, five minute TTL) and for the long-lived token. When no token is
//! configured the API is open, exactly as the legacy single-user default.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use pfl_domain::*;
use pfl_workflow::{WorkflowService, export_docx, export_html, export_markdown};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;

pub const DOWNLOAD_TOKEN_TTL_MS: u64 = 5 * 60 * 1000;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

/// Constant-time comparison, matching the legacy guard's timing-safe check.
fn timing_safe_equal(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct ApiState {
    pub service: Arc<WorkflowService>,
    token: Option<String>,
    download_tokens: Mutex<HashMap<String, u64>>,
}

impl ApiState {
    pub fn new(service: Arc<WorkflowService>, token: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            service,
            token: token.filter(|t| !t.is_empty()),
            download_tokens: Mutex::new(HashMap::new()),
        })
    }

    /// Single-use token valid for five minutes, pruning expired entries.
    pub fn generate_download_token(&self) -> String {
        let token = format!(
            "{:032x}{:032x}",
            now_ms() as u128,
            self as *const Self as usize as u128
        );
        let expiry = now_ms() + DOWNLOAD_TOKEN_TTL_MS;
        let mut tokens = self
            .download_tokens
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        tokens.retain(|_, exp| *exp > now_ms());
        tokens.insert(token.clone(), expiry);
        token
    }

    /// Consumes a one-time token, or accepts the long-lived token unchanged.
    fn accept_query_token(&self, candidate: &str) -> bool {
        {
            let mut tokens = self
                .download_tokens
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(expiry) = tokens.remove(candidate)
                && expiry > now_ms()
            {
                return true;
            }
        }
        match &self.token {
            Some(token) => timing_safe_equal(candidate, token),
            None => false,
        }
    }
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({
            "message": message,
            "error": "Unauthorized",
            "statusCode": 401,
        })),
    )
        .into_response()
}

async fn guard(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(token) = state.token.clone() else {
        return next.run(request).await;
    };
    let query_token = request.uri().query().and_then(|query| {
        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == "token").then(|| value.to_string())
        })
    });
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, value) = value.split_once(' ')?;
            (scheme == "Bearer").then(|| value.to_string())
        });
    if let Some(bearer) = &bearer
        && timing_safe_equal(bearer, &token)
    {
        return next.run(request).await;
    }
    if let Some(candidate) = &query_token
        && state.accept_query_token(candidate)
    {
        return next.run(request).await;
    }
    if bearer.is_none() && query_token.is_none() {
        return unauthorized("Authentication required. Set Authorization: Bearer <token> header.");
    }
    unauthorized("Invalid authentication token.")
}

#[derive(Serialize)]
struct Health {
    status: &'static str,
    service: &'static str,
    timestamp: String,
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok",
        // Compatible identity: the tray/DSH health monitor keys on this shape.
        service: "patentforge-backend",
        timestamp: format!("{}.{:03}Z", iso_seconds(), now_ms() % 1000),
    })
}

fn iso_seconds() -> String {
    // Minimal UTC ISO-8601 without pulling a calendar crate.
    let secs = now_ms() / 1000;
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

/// Howard Hinnant's civil_from_days.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

async fn download_token(State(state): State<Arc<ApiState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "token": state.generate_download_token() }))
}

type ApiResult<T> = std::result::Result<Json<T>, ApiError>;

struct ApiError(Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self.0 {
            Error::NotFound(_) => (StatusCode::NOT_FOUND, self.0.to_string()),
            Error::Invalid(_) => (StatusCode::BAD_REQUEST, self.0.to_string()),
            Error::Conflict(_) => (StatusCode::CONFLICT, self.0.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.0.to_string()),
        };
        (
            status,
            Json(serde_json::json!({ "message": message, "statusCode": status.as_u16() })),
        )
            .into_response()
    }
}

impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self(error)
    }
}

async fn projects(State(state): State<Arc<ApiState>>) -> ApiResult<Vec<Project>> {
    Ok(Json(state.service.projects().await?))
}

async fn project(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> ApiResult<Project> {
    Ok(Json(state.service.project(ProjectId(id)).await?))
}

async fn history(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<RunSnapshot>> {
    Ok(Json(state.service.history(ProjectId(id)).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IntakeBody {
    intake: Intake,
}

async fn save_intake(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Json(body): Json<IntakeBody>,
) -> ApiResult<Project> {
    Ok(Json(
        state
            .service
            .save_intake(ProjectId(id), body.intake)
            .await?,
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateProjectBody {
    intake: Intake,
}

async fn create_project(
    State(state): State<Arc<ApiState>>,
    Json(body): Json<CreateProjectBody>,
) -> ApiResult<Project> {
    Ok(Json(state.service.create_project(body.intake).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartBody {
    project_id: String,
    workflow: Option<String>,
    idempotency_key: String,
    #[serde(default)]
    authorization: Authorization,
    #[serde(default)]
    prior_art: Vec<PriorArtItem>,
}

async fn start(
    State(state): State<Arc<ApiState>>,
    Json(body): Json<StartBody>,
) -> ApiResult<RunSnapshot> {
    let kind = match body.workflow.as_deref() {
        None => WorkflowKind::Feasibility,
        Some(value) => WorkflowKind::parse(value)
            .ok_or_else(|| Error::Invalid(format!("unknown workflow {value}")))?,
    };
    let run = state
        .service
        .start_workflow_with_prior_art(
            ProjectId(body.project_id),
            kind,
            body.idempotency_key,
            body.authorization,
            body.prior_art,
        )
        .await?;
    Ok(Json(run))
}

async fn run(State(state): State<Arc<ApiState>>, Path(id): Path<String>) -> ApiResult<RunSnapshot> {
    Ok(Json(state.service.run(RunId(id)).await?))
}

#[derive(Deserialize)]
struct EventsQuery {
    #[serde(default)]
    after: u64,
}

async fn events(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> ApiResult<Vec<RunEvent>> {
    Ok(Json(
        state
            .service
            .events(Subscribe {
                run_id: RunId(id),
                after_sequence: query.after,
            })
            .await?,
    ))
}

async fn report(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let saved = state.service.report(RunId(id)).await?;
    Ok(Json(serde_json::json!({
        "runId": saved.run_id,
        "version": saved.version,
        "markdown": saved.markdown,
    })))
}

async fn resume(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> ApiResult<RunSnapshot> {
    Ok(Json(state.service.resume(RunId(id)).await?))
}

async fn cancel(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
) -> ApiResult<RunSnapshot> {
    Ok(Json(state.service.cancel(RunId(id)).await?))
}

#[derive(Deserialize)]
struct ExportQuery {
    format: Option<String>,
}

async fn export(
    State(state): State<Arc<ApiState>>,
    Path(id): Path<String>,
    Query(query): Query<ExportQuery>,
) -> std::result::Result<Response, ApiError> {
    let run = state.service.run(RunId(id)).await?;
    let format = query.format.unwrap_or_else(|| "markdown".into());
    let (content_type, body, extension) = match format.as_str() {
        "markdown" | "md" => (
            "text/markdown; charset=utf-8",
            export_markdown(&run)?.into_bytes(),
            "md",
        ),
        "html" => (
            "text/html; charset=utf-8",
            export_html(&run)?.into_bytes(),
            "html",
        ),
        "docx" | "word" => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            export_docx(&run)?,
            "docx",
        ),
        other => {
            return Err(ApiError(Error::Invalid(format!(
                "unknown export format {other}"
            ))));
        }
    };
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"report-v{}.{extension}\"",
                    run.version
                ),
            ),
        ],
        body,
    )
        .into_response())
}

pub fn router(state: Arc<ApiState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/health/download-token", post(download_token))
        .route("/api/projects", get(projects).post(create_project))
        .route("/api/projects/{id}", get(project))
        .route("/api/projects/{id}/intake", post(save_intake))
        .route("/api/projects/{id}/history", get(history))
        .route("/api/runs", post(start))
        .route("/api/runs/{id}", get(run))
        .route("/api/runs/{id}/events", get(events))
        .route("/api/runs/{id}/report", get(report))
        .route("/api/runs/{id}/resume", post(resume))
        .route("/api/runs/{id}/cancel", post(cancel))
        .route("/api/runs/{id}/export", get(export))
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

/// Binds the given address and serves until the returned future is dropped.
/// Lite never claims the legacy 4320-4324 range by default.
pub async fn serve(addr: &str, state: Arc<ApiState>) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}
