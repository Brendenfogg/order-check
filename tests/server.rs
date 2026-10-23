use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use order_check::menu::Menu;
use order_check::mic::AudioSource;
use order_check::records::Records;
use order_check::server::{AppState, router};
use order_check::session::{SessionConfig, Sessions};
use order_check::stt::FakeTranscriber;

struct QuietMic;

impl AudioSource for QuietMic {
    fn start(&mut self, _sink: Sender<Vec<f32>>) -> anyhow::Result<()> {
        Ok(())
    }
    fn stop(&mut self) {}
}

fn app() -> axum::Router {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let menu = Arc::new(Menu::load(&root.join("menus/chick-fil-a.toml")).unwrap());
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let sessions = Sessions::new(
        menu,
        Arc::new(FakeTranscriber::new(&["a spicy chicken sandwich"])),
        Records::open_in_memory().unwrap(),
        Box::new(QuietMic),
        SessionConfig {
            standards_on: false,
            demo_transcript: false,
        },
        tx.clone(),
    );
    router(AppState {
        sessions: Arc::new(sessions),
        events: tx,
        ready_after: Duration::from_millis(10),
        web_dir: root.join("web"),
    })
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn menu_endpoint() {
    let (status, body) = call(&app(), "GET", "/api/menu", serde_json::Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn order_flow_and_no_transcript_in_response() {
    let app = app();
    let (status, body) = call(
        &app,
        "POST",
        "/api/orders",
        serde_json::json!({"mode": "drive_thru", "label": "Navy SUV"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let n = body["order_num"].as_u64().unwrap();
    assert_eq!(n, 1001);

    let ticket = serde_json::json!({"lines": [{"item": "cookie", "qty": 1}]});
    let (status, body) = call(&app, "POST", &format!("/api/orders/{n}/finalize"), ticket).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.get("transcript").is_none(), "{body}");
    assert!(body["result"]["outcome"].is_string());

    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/orders/{n}/send"),
        serde_json::json!({"as_rung": false}),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, dash) = call(
        &app,
        "GET",
        "/api/dashboard?hours=24",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dash["orders"], 1);
}

#[tokio::test]
async fn finalize_rejects_unknown_item() {
    let app = app();
    let (_, body) = call(
        &app,
        "POST",
        "/api/orders",
        serde_json::json!({"mode": "face2face", "label": "Ann"}),
    )
    .await;
    let n = body["order_num"].as_u64().unwrap();
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/orders/{n}/finalize"),
        serde_json::json!({"lines": [{"item": "big_mac", "qty": 1}]}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn unknown_order_is_404() {
    let (status, _) = call(
        &app(),
        "POST",
        "/api/orders/999/finalize",
        serde_json::json!({"lines": []}),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bad_mode_is_400() {
    let (status, _) = call(
        &app(),
        "POST",
        "/api/orders",
        serde_json::json!({"mode": "teleport", "label": "x"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
