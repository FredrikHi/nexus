-- The auth service keeps its own database. The Rust API never reads these
-- tables: it verifies signed tokens instead, which is what lets identity be
-- re-pointed at another provider without touching the domain schema.
--
-- Runs only when the data volume is first created.
CREATE DATABASE integration_observability_auth;
