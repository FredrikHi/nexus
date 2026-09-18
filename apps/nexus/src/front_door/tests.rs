//! The front door against real upstream servers on loopback ports.
//!
//! Each fake upstream echoes back what reached it, so the assertions check what
//! actually crossed the wire rather than what the proxy meant to send.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::connect_info::MockConnectInfo;
use axum::extract::Request;
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{any, get};
use axum::{Json, Router};
use http_body_util::BodyExt;
use tempfile::TempDir;
use tower::ServiceExt;

use super::Settings;

const PEER: ([u8; 4], u16) = ([203, 0, 113, 7], 50_000);

/// A built web app: index.html plus one fingerprinted bundle.
fn web_dir() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("index.html"),
        "<!doctype html><title>nexus</title>",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(
        dir.path().join("assets").join("app-3f9a.js"),
        "console.log(1)",
    )
    .unwrap();
    std::fs::write(dir.path().join("favicon.svg"), "<svg/>").unwrap();
    dir
}

/// Starts an upstream that answers every request with a JSON description of
/// it, labelled `name` so a test can tell which backend a request reached.
async fn echo_upstream(name: &'static str) -> String {
    let app = Router::new()
        // Under /api/v1 because the proxy forwards paths unchanged.
        .route(
            "/api/v1/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_secs(5)).await;
                "late"
            }),
        )
        .fallback(any(move |request: Request| async move {
            let (parts, body) = request.into_parts();
            let body = body.collect().await.unwrap().to_bytes();
            let headers: HashMap<String, String> = parts
                .headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect();
            let echo = serde_json::json!({
                "upstream": name,
                "method": parts.method.as_str(),
                "path": parts.uri.path(),
                "query": parts.uri.query(),
                "headers": headers,
                "body": String::from_utf8_lossy(&body),
            });
            // A hop-by-hop header on the way back, which must not survive.
            ([("keep-alive", "timeout=5")], Json(echo)).into_response()
        }));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr.to_string()
}

/// An address nothing listens on: bound, read, and released again.
async fn dead_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().to_string()
}

fn config(web: &Path, api: &str, auth: &str) -> Settings {
    Settings {
        web_dir: web.to_path_buf(),
        api_base: format!("http://{api}"),
        auth_base: format!("http://{auth}"),
        proxy_timeout: Duration::from_secs(60),
    }
}

/// The router as a service, with a fixed client address standing in for the
/// connection info a real listener records.
fn app(config: &Settings) -> Router {
    super::router(config, super::http_client()).layer(MockConnectInfo(SocketAddr::from(PEER)))
}

async fn send(app: Router, request: Request) -> (StatusCode, HeaderMap, Bytes) {
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, body)
}

fn get_request(uri: &str) -> Request {
    Request::builder()
        .uri(uri)
        .header(header::HOST, "nexus.example.com")
        .body(Body::empty())
        .unwrap()
}

fn json(body: &Bytes) -> serde_json::Value {
    serde_json::from_slice(body).unwrap()
}

// --- static files -------------------------------------------------------------

#[tokio::test]
async fn healthz_answers_without_any_backend() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, _, body) = send(app(&config), get_request("/healthz")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json(&body)["status"], "ok");
}

#[tokio::test]
async fn root_serves_index_uncached() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, headers, body) = send(app(&config), get_request("/")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("<title>nexus</title>"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn client_side_route_falls_back_to_index() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, headers, body) = send(app(&config), get_request("/systems/42/components")).await;

    assert_eq!(status, StatusCode::OK);
    assert!(String::from_utf8_lossy(&body).contains("<title>nexus</title>"));
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
}

#[tokio::test]
async fn other_root_files_are_served_as_themselves() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, headers, body) = send(app(&config), get_request("/favicon.svg")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body[..], b"<svg/>");
    assert!(headers.get(header::CACHE_CONTROL).is_none());
}

#[tokio::test]
async fn fingerprinted_assets_are_cached_for_a_year() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, headers, body) = send(app(&config), get_request("/assets/app-3f9a.js")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(&body[..], b"console.log(1)");
    assert_eq!(
        headers[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
}

#[tokio::test]
async fn missing_asset_is_404_not_index_and_not_cached() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, headers, body) = send(app(&config), get_request("/assets/gone-1234.js")).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!String::from_utf8_lossy(&body).contains("<title>"));
    assert!(headers.get(header::CACHE_CONTROL).is_none());
}

// --- proxying -------------------------------------------------------------------

#[tokio::test]
async fn api_requests_reach_the_api_with_path_query_and_forwarding_headers() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let auth = echo_upstream("auth").await;
    let config = config(web.path(), &api, &auth);

    let (status, _, body) =
        send(app(&config), get_request("/api/v1/systems?limit=5&q=a%20b")).await;

    assert_eq!(status, StatusCode::OK);
    let echo = json(&body);
    assert_eq!(echo["upstream"], "api");
    assert_eq!(echo["path"], "/api/v1/systems");
    assert_eq!(echo["query"], "limit=5&q=a%20b");
    assert_eq!(echo["headers"]["host"], "nexus.example.com");
    assert_eq!(echo["headers"]["x-forwarded-for"], "203.0.113.7");
    assert_eq!(echo["headers"]["x-real-ip"], "203.0.113.7");
    assert_eq!(echo["headers"]["x-forwarded-proto"], "http");
    assert_eq!(echo["headers"]["x-forwarded-host"], "nexus.example.com");
}

#[tokio::test]
async fn auth_requests_reach_the_auth_service() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let auth = echo_upstream("auth").await;
    let config = config(web.path(), &api, &auth);

    let (status, _, body) = send(app(&config), get_request("/api/auth/get-session")).await;

    assert_eq!(status, StatusCode::OK);
    let echo = json(&body);
    assert_eq!(echo["upstream"], "auth");
    assert_eq!(echo["path"], "/api/auth/get-session");
}

#[tokio::test]
async fn forwarding_headers_from_a_proxy_in_front_are_kept() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let config = config(web.path(), &api, &api);

    // What IIS terminating TLS in front of this process sends.
    let request = Request::builder()
        .uri("/api/v1/systems")
        .header(header::HOST, "127.0.0.1:5173")
        .header("x-forwarded-proto", "https")
        .header("x-forwarded-host", "nexus.brandkontoret.se")
        .header("x-forwarded-for", "10.0.0.20")
        .body(Body::empty())
        .unwrap();
    let (_, _, body) = send(app(&config), request).await;

    let echo = json(&body);
    assert_eq!(echo["headers"]["x-forwarded-proto"], "https");
    assert_eq!(
        echo["headers"]["x-forwarded-host"],
        "nexus.brandkontoret.se"
    );
    assert_eq!(echo["headers"]["x-forwarded-for"], "10.0.0.20, 203.0.113.7");
}

#[tokio::test]
async fn request_bodies_stream_through() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let config = config(web.path(), &api, &api);

    let payload = r#"{"events":[{"integration":"orders-to-stripe","outcome":"SUCCESS"}]}"#;
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/telemetry")
        .header(header::HOST, "nexus.example.com")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload))
        .unwrap();
    let (status, _, body) = send(app(&config), request).await;

    assert_eq!(status, StatusCode::OK);
    let echo = json(&body);
    assert_eq!(echo["method"], "POST");
    assert_eq!(echo["body"], payload);
}

#[tokio::test]
async fn hop_by_hop_headers_do_not_cross_in_either_direction() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let config = config(web.path(), &api, &api);

    let request = Request::builder()
        .uri("/api/v1/systems")
        .header(header::HOST, "nexus.example.com")
        .header(header::CONNECTION, "x-hop-secret")
        .header("x-hop-secret", "for this hop only")
        .header(header::PROXY_AUTHORIZATION, "Basic c2VjcmV0")
        .body(Body::empty())
        .unwrap();
    let (_, headers, body) = send(app(&config), request).await;

    let echo = json(&body);
    assert!(echo["headers"].get("x-hop-secret").is_none());
    assert!(echo["headers"].get("proxy-authorization").is_none());
    assert!(headers.get("keep-alive").is_none());
}

#[tokio::test]
async fn unreachable_backend_is_a_502_in_the_api_error_shape() {
    let web = web_dir();
    let dead = dead_upstream().await;
    let config = config(web.path(), &dead, &dead);

    let (status, _, body) = send(app(&config), get_request("/api/v1/systems")).await;

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(json(&body)["error"]["code"], "BAD_GATEWAY");
}

#[tokio::test]
async fn slow_backend_is_a_504() {
    let web = web_dir();
    let api = echo_upstream("api").await;
    let mut config = config(web.path(), &api, &api);
    config.proxy_timeout = Duration::from_millis(200);

    let (status, _, body) = send(app(&config), get_request("/api/v1/slow")).await;

    assert_eq!(status, StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(json(&body)["error"]["code"], "GATEWAY_TIMEOUT");
}
