//! The front door: serves the web app and forwards `/api/auth` and `/api/v1`,
//! the job nginx does inside the web image. The browser sees one origin.

mod proxy;
mod web;

#[cfg(test)]
mod tests;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::response::Response;
use axum::routing::{any, get};
use axum::{Json, Router};
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub use proxy::{http_client, HttpClient};

use proxy::Upstream;

/// What the front door needs to know, and nothing else. Deliberately not the
/// whole `Config`: the front door does not care how the backends were started,
/// and its tests should not have to fake a database URL to check a header.
#[derive(Debug, Clone)]
pub struct Settings {
    pub web_dir: PathBuf,
    /// `http://host:port`, no trailing slash.
    pub api_base: String,
    /// `http://host:port`, no trailing slash.
    pub auth_base: String,
    /// How long a backend may take to start answering; nginx's
    /// `proxy_read_timeout`.
    pub proxy_timeout: Duration,
}

impl Settings {
    pub fn new(web_dir: PathBuf, api: SocketAddr, auth: SocketAddr) -> Self {
        Self {
            web_dir,
            api_base: format!("http://{api}"),
            auth_base: format!("http://{auth}"),
            proxy_timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Clone)]
struct AppState {
    api: Upstream,
    auth: Upstream,
}

pub fn router(settings: &Settings, client: HttpClient) -> Router {
    let state = AppState {
        api: Upstream {
            client: client.clone(),
            base: settings.api_base.clone(),
            timeout: settings.proxy_timeout,
        },
        auth: Upstream {
            client,
            base: settings.auth_base.clone(),
            timeout: settings.proxy_timeout,
        },
    };

    Router::new()
        // Answers without touching a backend, so "the front door is up" can be
        // told apart from "everything behind it is up".
        .route("/healthz", get(healthz))
        // Identity, then the domain API. Neither prefix can shadow the other,
        // so unlike nginx's longest-prefix rule the order here does not matter.
        .route("/api/auth/{*rest}", any(to_auth))
        .route("/api/v1/{*rest}", any(to_api))
        .with_state(state)
        .merge(web::router(&settings.web_dir))
        // Skips responses that already carry a Content-Encoding, so it never
        // double-compresses something a backend compressed itself.
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
}

async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

async fn to_api(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    proxy::forward(&state.api, peer, request).await
}

async fn to_auth(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    proxy::forward(&state.auth, peer, request).await
}
