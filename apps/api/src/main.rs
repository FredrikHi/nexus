mod api_keys;
mod auth;
mod components;
mod domain;
mod environments;
mod error;
mod extract;
mod health;
mod incidents;
mod integration_health;
mod integration_types;
mod integrations;
mod openapi;
mod organizations;
mod systems;
mod teams;
mod telemetry;
mod traces;
mod util;

use std::net::SocketAddr;

use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::trace::TraceLayer;
use utoipa_swagger_ui::SwaggerUi;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Shared application state handed to every request handler.
/// `Clone` is cheap here: PgPool is an Arc-backed handle, so cloning
/// just bumps a reference count, it does not open new connections.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Public keys of the auth service, fetched once and cached. Token
    /// validation is local, so it costs no network round trip per request.
    pub jwks: auth::JwksCache,
    /// Who tokens must be issued by, and who they must be issued for. Both are
    /// verified on every request: a valid signature from the right key is not
    /// enough if the token was minted for a different application.
    pub auth_issuer: String,
    pub auth_audience: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load variables from a local .env file if present. In production the
    // orchestrator sets real env vars, so this is a harmless no-op there.
    dotenvy::dotenv().ok();

    // 1. Structured logging. EnvFilter lets you control verbosity via RUST_LOG.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "integration_api=debug,tower_http=debug,info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 2. Configuration from the environment, with dev-friendly defaults.
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/integration_observability".to_string()
    });
    // Where the auth service publishes its public keys, and the issuer and
    // audience every token must claim. Read once at startup like everything
    // else, so changing them needs a restart.
    let auth_issuer = std::env::var("AUTH_ISSUER")
        .unwrap_or_else(|_| "http://localhost:3010".to_string());
    let auth_audience = std::env::var("AUTH_AUDIENCE")
        .unwrap_or_else(|_| "integration-observability-api".to_string());
    let jwks_url = std::env::var("AUTH_JWKS_URL")
        .unwrap_or_else(|_| format!("{auth_issuer}/api/auth/jwks"));

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    // 3. Build the connection pool. `connect_lazy` does NOT connect yet;
    //    the first query opens a real connection. This means the API can
    //    boot even if Postgres is briefly unavailable.
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect_lazy(&database_url)?;

    // Apply any pending migrations at startup. `migrate!` embeds the SQL
    // files from ./migrations into the binary at compile time, so the
    // deployed executable carries its own schema history (like EF Core's
    // Database.Migrate()). SQLx takes an advisory lock, so two instances
    // booting at once won't double-apply.
    sqlx::migrate!("./migrations").run(&db).await?;
    tracing::info!("database migrations applied");

    // Extend the telemetry partition window so ingest always has somewhere to
    // write. Idempotent, and cheap enough to do on every boot.
    telemetry::ensure_partitions(&db).await?;
    tracing::info!("telemetry partitions ensured");

    // Health is evaluated on a timer rather than on read, so a status change
    // is noticed while nobody is watching. Only one instance evaluates at a
    // time; the others skip the pass.
    if integration_health::worker_enabled() {
        let config = integration_health::WorkerConfig::from_env();
        tracing::info!(
            interval_secs = config.interval.as_secs(),
            retention_days = config.retention_days,
            "starting health evaluation worker"
        );
        integration_health::spawn_worker(db.clone(), config);
    } else {
        tracing::info!("health evaluation worker disabled");
    }

    let state = AppState {
        db,
        jwks: auth::JwksCache::new(jwks_url),
        auth_issuer,
        auth_audience,
    };

    // 4. The router: URL -> handler. This is your ASP.NET endpoint map.
    let app = Router::new()
        .route("/api/v1/health", get(health::liveness))
        .route("/api/v1/health/ready", get(health::readiness))
        .merge(environments::router())
        .merge(systems::router())
        .merge(components::router())
        .merge(integrations::router())
        .merge(api_keys::router())
        .merge(telemetry::router())
        .merge(traces::router())
        .merge(integration_health::router())
        .merge(incidents::router())
        .merge(organizations::router())
        .merge(teams::router())
        .merge(integration_types::router())
        // Swagger UI at /swagger-ui, reading the document it serves at
        // /api-docs/openapi.json. The assets are vendored into the binary, so
        // neither the build nor the running process needs network access.
        .merge(
            SwaggerUi::new("/swagger-ui")
                .url("/api-docs/openapi.json", openapi::spec()),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // 5. Bind a TCP socket and serve. `.await` yields to Tokio until ready.
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!("listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
