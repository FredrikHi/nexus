use axum::{extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::AppState;

/// One row of the `environments` table.
///
/// `query_as!` builds this struct directly, matching each result column to the
/// field of the same name. It does not go through `FromRow`: the macro already
/// knows the exact shape of the result at compile time.
#[derive(Debug, Serialize, ToSchema)]
pub struct Environment {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>, // nullable column  ->  Option<T>  (like string?)
    pub is_production: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Repository function: run one query, map every row into an `Environment`.
///
/// `query_as!` is *compile-time*-checked: the statement is verified against a
/// real Postgres schema while the crate builds, so a renamed column is a build
/// error rather than a runtime 500. The `?` propagates any `sqlx::Error`.
pub async fn list_environments(pool: &sqlx::PgPool) -> Result<Vec<Environment>, sqlx::Error> {
    let environments = sqlx::query_as!(
        Environment,
        r#"SELECT id, organization_id, name, slug, description,
                  is_production, created_at, updated_at
           FROM environments
           ORDER BY is_production, name"#
    )
    .fetch_all(pool)
    .await?;

    Ok(environments)
}

/// HTTP handler. Straight-through for now; in Phase 2 we wrap this pattern in
/// the handler -> service -> repository layering.
#[utoipa::path(
    get,
    path = "/api/v1/environments",
    tag = "Reference data",
    responses((status = 200, description = "All environments", body = Vec<Environment>))
)]
pub async fn list_environments_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<Environment>>, StatusCode> {
    match list_environments(&state.db).await {
        Ok(environments) => Ok(Json(environments)),
        Err(err) => {
            tracing::error!(error = %err, "failed to list environments");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `#[sqlx::test]` spins up a *fresh, isolated* database, runs every
    /// migration into it (including the seed), hands us a connected pool, and
    /// drops the database afterwards. Think test containers, but built in.
    #[sqlx::test]
    async fn seed_creates_the_four_environments(pool: sqlx::PgPool) -> sqlx::Result<()> {
        let environments = list_environments(&pool).await?;

        assert_eq!(environments.len(), 4);
        assert!(environments
            .iter()
            .any(|e| e.slug == "production" && e.is_production));
        Ok(())
    }
}
