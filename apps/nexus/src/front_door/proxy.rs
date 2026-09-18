//! Forwards a request to one of the backends, the way nginx's `proxy_pass`
//! did in the web image.
//!
//! Bodies stream through in both directions. Nothing is buffered, so a large
//! telemetry batch costs no more memory here than it does in the API.

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::{self, HeaderMap, HeaderName, HeaderValue};
use axum::http::{StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

/// Connection-pooling HTTP client, shared by every request. Cloning it is
/// cheap: the pool lives behind an `Arc` inside.
pub type HttpClient = Client<HttpConnector, Body>;

pub fn http_client() -> HttpClient {
    let mut connector = HttpConnector::new();
    // Windows does not refuse a connection to a closed loopback port at once:
    // it retries for about two seconds first. While a backend restarts, every
    // request would hang that long before failing. A backend on this machine
    // that is up accepts in microseconds, so a short limit only ever cuts off
    // a backend that is not there.
    connector.set_connect_timeout(Some(Duration::from_millis(500)));
    Client::builder(TokioExecutor::new()).build(connector)
}

/// One backend: where it is and how long it gets.
#[derive(Clone)]
pub struct Upstream {
    pub client: HttpClient,
    /// `http://host:port`, no trailing slash.
    pub base: String,
    pub timeout: Duration,
}

static X_FORWARDED_FOR: HeaderName = HeaderName::from_static("x-forwarded-for");
static X_FORWARDED_HOST: HeaderName = HeaderName::from_static("x-forwarded-host");
static X_FORWARDED_PROTO: HeaderName = HeaderName::from_static("x-forwarded-proto");
static X_REAL_IP: HeaderName = HeaderName::from_static("x-real-ip");

/// Headers that describe one hop, not the request. Passing them on would let
/// a client's `Connection: close` or `Transfer-Encoding` steer the connection
/// between this process and the backend.
const HOP_BY_HOP: [HeaderName; 8] = [
    header::CONNECTION,
    HeaderName::from_static("keep-alive"),
    header::PROXY_AUTHENTICATE,
    header::PROXY_AUTHORIZATION,
    header::TE,
    header::TRAILER,
    header::TRANSFER_ENCODING,
    header::UPGRADE,
];

pub async fn forward(upstream: &Upstream, peer: SocketAddr, mut request: Request) -> Response {
    let path_and_query = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: Uri = match format!("{}{}", upstream.base, path_and_query).parse() {
        Ok(uri) => uri,
        // Only reachable with a path the incoming parser accepted and this one
        // does not. Not the client's fault in any way they could act on.
        Err(err) => {
            tracing::warn!(error = %err, path = path_and_query, "could not build upstream uri");
            return error(
                StatusCode::BAD_GATEWAY,
                "BAD_GATEWAY",
                "the request could not be forwarded",
            );
        }
    };
    *request.uri_mut() = uri;

    let headers = request.headers_mut();
    strip_hop_by_hop(headers);
    set_forwarded(headers, peer);

    // The timeout covers waiting for the response to start, like nginx's.
    // A slow but steady body stream after that is left alone.
    match tokio::time::timeout(upstream.timeout, upstream.client.request(request)).await {
        Ok(Ok(response)) => {
            let (mut parts, body) = response.into_parts();
            strip_hop_by_hop(&mut parts.headers);
            Response::from_parts(parts, Body::new(body))
        }
        Ok(Err(err)) => {
            tracing::warn!(upstream = %upstream.base, error = %err, "upstream unreachable");
            error(
                StatusCode::BAD_GATEWAY,
                "BAD_GATEWAY",
                "a Nexus service is not responding",
            )
        }
        Err(_) => {
            tracing::warn!(upstream = %upstream.base, timeout = ?upstream.timeout, "upstream timed out");
            error(
                StatusCode::GATEWAY_TIMEOUT,
                "GATEWAY_TIMEOUT",
                "a Nexus service took too long to respond",
            )
        }
    }
}

fn strip_hop_by_hop(headers: &mut HeaderMap) {
    // A Connection header can name further headers that are hop-by-hop for
    // this one message. Collected first: the map cannot be changed while the
    // value borrowed from it is still being read.
    let named: Vec<HeaderName> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| HeaderName::from_bytes(name.trim().as_bytes()).ok())
        .collect();

    for name in named.iter().chain(HOP_BY_HOP.iter()) {
        headers.remove(name);
    }
}

/// What nginx's `proxy_set_header` lines did, with one difference: an
/// `X-Forwarded-Proto` that arrives is kept rather than overwritten with this
/// hop's own scheme. Behind IIS or another TLS terminator the browser used
/// https, and the auth service needs to know that to mark cookies Secure.
fn set_forwarded(headers: &mut HeaderMap, peer: SocketAddr) {
    let peer_ip = peer.ip().to_string();

    let forwarded_for = match headers.get(&X_FORWARDED_FOR).and_then(|v| v.to_str().ok()) {
        Some(existing) => format!("{existing}, {peer_ip}"),
        None => peer_ip.clone(),
    };
    insert(headers, &X_FORWARDED_FOR, &forwarded_for);
    insert(headers, &X_REAL_IP, &peer_ip);

    if !headers.contains_key(&X_FORWARDED_PROTO) {
        headers.insert(X_FORWARDED_PROTO.clone(), HeaderValue::from_static("http"));
    }
    if !headers.contains_key(&X_FORWARDED_HOST) {
        if let Some(host) = headers.get(header::HOST).cloned() {
            headers.insert(X_FORWARDED_HOST.clone(), host);
        }
    }
}

fn insert(headers: &mut HeaderMap, name: &HeaderName, value: &str) {
    // An IP address or a list of them is always a valid header value; the
    // fallible conversion is the type system not knowing that.
    if let Ok(value) = HeaderValue::from_str(value) {
        headers.insert(name.clone(), value);
    }
}

/// The API's own error envelope, so the web app reads a proxy failure the same
/// way it reads any other.
fn error(status: StatusCode, code: &str, message: &str) -> Response {
    let body = serde_json::json!({ "error": { "code": code, "message": message } });
    (status, Json(body)).into_response()
}
