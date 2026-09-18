//! Serves the built web app: what nginx's `root`, `try_files` and cache
//! headers did in the web image.

use std::path::Path;

use axum::extract::Request;
use axum::http::header::{self, HeaderValue};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};

/// Two routes' worth of files:
///
/// - `/assets/*` are Vite's fingerprinted bundles. Cached for a year, and a
///   missing one is a 404: answering with index.html instead would hand the
///   browser HTML where it expected JavaScript, and the error it shows then
///   points nowhere near the cause.
/// - Everything else is a file if one exists, and otherwise index.html, so
///   React Router can handle a deep link or a refresh on a client-side route.
pub fn router(web_dir: &Path) -> Router {
    let assets = Router::new()
        .fallback_service(ServeDir::new(web_dir.join("assets")))
        .layer(middleware::from_fn(cache_forever));

    let spa = ServeDir::new(web_dir).fallback(ServeFile::new(web_dir.join("index.html")));

    Router::new()
        .nest("/assets", assets)
        .fallback_service(spa)
        .layer(middleware::from_fn(never_cache_html))
}

async fn cache_forever(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    // Only what was actually found. A 404 cached for a year would outlive the
    // deployment that fixes it.
    if response.status().is_success() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}

/// index.html is the one file whose name does not change between releases,
/// and it names the fingerprinted bundles. A cached copy keeps a browser
/// asking for the previous release's files after an upgrade.
///
/// Keyed on content type rather than path: the SPA fallback serves index.html
/// under every client-side route, and every one of those has to be uncached.
async fn never_cache_html(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let is_html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/html"));
    if is_html {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}
