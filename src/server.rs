//! The local HTTP + WebSocket API the POS and dashboard use.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;

use crate::menu::Menu;
use crate::records::Dashboard;
use crate::session::{Event, Sessions};
use crate::ticket::Ticket;
use crate::verify::CheckResult;

#[derive(Clone)]
pub struct AppState {
    pub sessions: Arc<Sessions>,
    pub events: broadcast::Sender<Event>,
    pub ready_after: Duration,
    pub web_dir: PathBuf,
}

pub fn router(state: AppState) -> axum::Router {
    let web = ServeDir::new(&state.web_dir);
    Router::new()
        .route("/api/menu", get(menu))
        .route("/api/orders", post(start_order))
        .route("/api/orders/{n}/finalize", post(finalize))
        .route("/api/orders/{n}/send", post(send))
        .route("/api/orders/{n}/cancel", post(cancel))
        .route("/api/dashboard", get(dashboard))
        .route("/ws", get(ws))
        .fallback_service(web)
        .with_state(state)
}

enum ApiError {
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:#}")),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError::Internal(e)
    }
}

fn require_order(state: &AppState, n: u32) -> Result<(), ApiError> {
    if state.sessions.has_transcript(n) {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("no open order {n}")))
    }
}

async fn menu(State(state): State<AppState>) -> Json<Menu> {
    Json(state.sessions.menu().clone())
}

#[derive(Deserialize)]
struct StartOrder {
    mode: String,
    #[serde(default)]
    label: String,
}

async fn start_order(
    State(state): State<AppState>,
    Json(body): Json<StartOrder>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !matches!(body.mode.as_str(), "drive_thru" | "face2face") {
        return Err(ApiError::BadRequest(format!(
            "mode must be drive_thru or face2face, got {}",
            body.mode
        )));
    }
    let n = state.sessions.start(&body.mode, &body.label)?;
    Ok(Json(serde_json::json!({ "order_num": n })))
}

/// The check result as the POS sees it. The transcript stays on the server.
#[derive(Serialize)]
struct FinalizeResponse {
    order_num: u32,
    result: CheckResult,
    standards: Option<BTreeMap<String, bool>>,
    verify_ms: u32,
}

async fn finalize(
    State(state): State<AppState>,
    Path(n): Path<u32>,
    Json(ticket): Json<Ticket>,
) -> Result<Json<FinalizeResponse>, ApiError> {
    require_order(&state, n)?;
    ticket
        .validate(state.sessions.menu())
        .map_err(|e| ApiError::BadRequest(format!("{e:#}")))?;
    let sessions = state.sessions.clone();
    let report = tokio::task::spawn_blocking(move || sessions.finalize(n, &ticket))
        .await
        .map_err(|e| ApiError::Internal(e.into()))??;
    Ok(Json(FinalizeResponse {
        order_num: n,
        result: report.result,
        standards: report.standards,
        verify_ms: report.verify_ms,
    }))
}

#[derive(Deserialize)]
struct SendBody {
    #[serde(default)]
    as_rung: bool,
}

async fn send(
    State(state): State<AppState>,
    Path(n): Path<u32>,
    Json(body): Json<SendBody>,
) -> Result<StatusCode, ApiError> {
    require_order(&state, n)?;
    state
        .sessions
        .send(n, body.as_rung)
        .map_err(|e| ApiError::BadRequest(format!("{e:#}")))?;
    // Stand-in for the kitchen expo screen marking the order ready.
    let sessions = state.sessions.clone();
    let wait = state.ready_after;
    tokio::spawn(async move {
        tokio::time::sleep(wait).await;
        sessions.ready(n);
    });
    Ok(StatusCode::NO_CONTENT)
}

async fn cancel(State(state): State<AppState>, Path(n): Path<u32>) -> StatusCode {
    state.sessions.cancel(n);
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct DashQuery {
    hours: Option<i64>,
}

async fn dashboard(
    State(state): State<AppState>,
    Query(q): Query<DashQuery>,
) -> Result<Json<Dashboard>, ApiError> {
    let hours = q.hours.unwrap_or(24).clamp(1, 24 * 90);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    Ok(Json(
        state
            .sessions
            .records()
            .dashboard(state.sessions.menu(), now - hours * 3600)?,
    ))
}

async fn ws(State(state): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    let mut rx = state.events.subscribe();
    upgrade.on_upgrade(move |mut socket: WebSocket| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let Ok(text) = serde_json::to_string(&event) else {
                        continue;
                    };
                    if socket.send(Message::Text(text.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
