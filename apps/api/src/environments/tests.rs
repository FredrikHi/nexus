use sqlx::PgPool;
use uuid::Uuid;

use super::service;

const DEFAULT_ORG: Uuid = Uuid::from_u128(1);

/// The seed migration gives the bootstrap organization the standard four.
#[sqlx::test]
async fn seed_creates_the_four_environments(pool: PgPool) {
    let environments = service::list(&pool, DEFAULT_ORG).await.expect("list");

    assert_eq!(environments.len(), 4);
    assert!(environments
        .iter()
        .any(|e| e.slug == "production" && e.is_production));
    // Production sorts last, so the list reads development-first.
    assert_eq!(environments.last().expect("one").slug, "production");
}
