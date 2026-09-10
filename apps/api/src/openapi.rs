//! Assembles the OpenAPI document from the per-feature slices.
//!
//! Each feature keeps its handlers private and publishes a small `openapi()`
//! instead, so this module never reaches into another module's internals. The
//! root document owns only what is genuinely global: the title, the tag list,
//! and the endpoints that live outside a feature module.

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Integration Observability Platform API",
        version = env!("CARGO_PKG_VERSION"),
        description = "Systems, the components inside them, and the integrations between them.\n\n\
                       Every error response shares one envelope: `{\"error\": {\"code\", \"message\"}}`. \
                       Detail behind a 5xx is redacted and logged server-side.",
    ),
    paths(
        crate::health::liveness,
        crate::health::readiness,
        crate::environments::list_environments_handler,
    ),
    tags(
        (name = "Systems", description = "Deployable units and external services."),
        (name = "Components", description = "The parts inside a system. Integrations connect these, not systems."),
        (name = "Integrations", description = "Directed edges between two components."),
        (name = "Reference data", description = "Environments and the integration type catalogue."),
        (name = "Telemetry", description = "Recorded calls across integrations: ingest and query."),
        (name = "API keys", description = "Credentials that authenticate telemetry ingestion."),
        (name = "Health", description = "Liveness and readiness probes."),
    ),
)]
struct RootApi;

/// The complete document served at `/api-docs/openapi.json`.
pub fn spec() -> utoipa::openapi::OpenApi {
    let mut doc = RootApi::openapi();
    doc.merge(crate::systems::openapi());
    doc.merge(crate::components::openapi());
    doc.merge(crate::integrations::openapi());
    doc.merge(crate::integration_types::openapi());
    doc.merge(crate::api_keys::openapi());
    doc.merge(crate::telemetry::openapi());
    doc
}
