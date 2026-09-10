# CLAUDE.md — Integration Observability Platform

> This file is the authoritative context for working on this repo. It was
> distilled from a long mentoring conversation. Where anything you're told
> elsewhere (an attached transcript, older notes) conflicts with this file,
> **follow this file.**

---

## 1. How to work with me

I'm Fredrik. I'm an experienced developer — C#, .NET, ASP.NET, React,
TypeScript, SQL/SQL Server, and controller/service/repository architectures are
second nature. **I am new to Rust**, and a primary goal of this project is to
learn Rust while building something real.

So act as an implementation partner *and* a mentor:

- **Teach Rust by comparison to C#/.NET.** When you introduce a Rust concept,
  anchor it to what I already know (`Result<T,E>` ≈ explicit success/error
  instead of exceptions; `Option<T>` ≈ nullable; `trait` ≈ interface; `impl` ≈
  implementing behavior; Tokio ≈ the async runtime; Axum handler ≈ ASP.NET
  minimal-API endpoint; SQLx ≈ Dapper-with-optional-compile-time-checks). These
  are aids, not claims of identity.
- **One step at a time.** Do not implement multiple phases at once. Complete a
  phase (or a sensible sub-step), let me verify it, explain it, and only then
  move on. If a step reveals an architectural problem, stop and explain rather
  than building on a shaky foundation.
- **Explain before you implement.** What are we building, why is it shaped this
  way, and what Rust concepts will I meet? Then build it.
- **Never silently make a major architectural decision.** If there are real
  alternatives, lay them out briefly and recommend one with reasoning.
- **Compile and test before handing work over.** Run `cargo build` / `cargo
  test` (and the frontend build where relevant). Read the actual compiler
  errors and fix them — don't hand me code you haven't built.
- **Spend teaching effort on Rust**, not on trivial React/TS/SQL I already know.
- End a meaningful Rust implementation with a short **"What I learned"** recap
  (a few lines mapping the new Rust bits to C# equivalents).
- Keep everything **production-quality**. Don't add complexity purely to teach;
  if two approaches are equally good, prefer the one that teaches a useful Rust
  concept without bloating the architecture.

Rough order to introduce Rust concepts as they become relevant: Cargo → structs
→ enums → Option → Result → pattern matching → ownership/borrowing/references →
traits → generics → lifetimes (only when needed) → async → Tokio → Axum → SQLx →
concurrency → background workers → error architecture → production hardening.

---

## 2. What we're building

An internal **Integration Observability Platform** — a single place to see the
company's systems, the components inside them, the integrations between them,
and the telemetry/health/incidents that show what's actually happening. Think
"Integration Control Center."

It must stay **generic and configurable** — never hard-code company-specific
integrations (SendGrid, Kivra, BankID, Pagero, EVRY, etc.) into the domain
model. Those are just data. The core concept chain is:

```
Systems → Components → Integrations → Telemetry → Traces → Health → Incidents
```

Full product spec and phase-by-phase plan live in `docs/` (drop them there if
not already present). This file is the working summary.

### Tech stack

- **Backend:** Rust, Axum 0.8, Tokio, SQLx 0.8 (Postgres), Serde, tracing,
  PostgreSQL 16.
- **Frontend:** React 19, TypeScript, Vite, Tailwind v4, TanStack Query, React
  Router 7. (I know this side well — keep explanations light here.)
- **Infra:** Docker + docker-compose for local; self-hosted Coolify for
  eventual deployment.

---

## 3. Current state (what already exists)

- **Phase 0 — foundation:** monorepo, Axum API with `/api/v1/health`
  (liveness) and `/api/v1/health/ready` (readiness, pings DB), React app shell
  with sidebar nav + a live "API healthy" pill, `docker-compose.yml`,
  `.env`/`.env.example` loaded via `dotenvy`. Done and running.
- **Phase 1 — database:** SQLx migrations `0001_core`, `0002_systems`,
  `0003_integrations`, `0004_seed` (14 tables). Seeded: default org, four
  environments (Development/Test/Staging/Production), 14 built-in integration
  types. `GET /api/v1/environments` proves the DB→struct→JSON path. Migrations
  run at startup via `sqlx::migrate!`. Done.
- **Phase 2a — architecture + systems:** shared `ApiError` type; full layered
  CRUD for `systems`. Done.
- **Phase 2b (in progress) — components:** full layered CRUD for `components`,
  including a `Query` extractor for `?system_id=` filtering. Done.
- **Build fix just applied:** added `"macros"` to the sqlx feature list (the
  `FromRow` derive and `#[sqlx::test]` need it) and removed an unused
  `use sqlx::error::DatabaseError;` in `src/error.rs`. The project should now
  compile.

### Immediate next work (Phase 2b/2c)

1. **`integrations` feature** — same layered pattern as `systems`/`components`.
   It references a source component, a destination component, an
   integration_type, and an environment. Its **service must validate those
   references exist with specific error messages** (not just lean on the generic
   FK-violation → 422 mapping) and **reject source == destination**.
2. **Extract `slugify`** (currently duplicated in `systems::service` and
   `components::service`) into a shared `crate::util` module — integrations is
   the third consumer, so the "rule of three" says extract now.
3. **OpenAPI docs** across systems/components/integrations (utoipa).
4. **Switch repositories to compile-time-checked SQL** (`query_as!`) with the
   offline `.sqlx` cache (`cargo sqlx prepare`) so the Docker build stays
   DB-independent. Explain the trade-off before doing it.

---

## 4. Repository layout

```
integration-observability/
├── apps/
│   ├── api/                     # Rust + Axum backend
│   │   ├── Cargo.toml
│   │   ├── migrations/          # 0001_core .. 0004_seed (SQLx)
│   │   └── src/
│   │       ├── main.rs          # bootstrap: tracing, pool, migrate!, router
│   │       ├── error.rs         # ApiError (shared)
│   │       ├── health.rs        # liveness + readiness
│   │       ├── environments.rs  # read-only demo endpoint
│   │       ├── systems/         # model/dto/repository/service/handlers/mod
│   │       └── components/      # model/dto/repository/service/handlers/mod
│   └── web/                     # React + Vite frontend
├── docker-compose.yml
├── .env.example
└── docs/                        # product spec + phase plan (reference)
```

**Feature module shape** (copy this for every new entity):

```
<feature>/
├── mod.rs          # declares submodules (private) + `pub fn router() -> Router<AppState>`
├── model.rs        # domain struct + enums + FromRow "Row" struct + into_domain()
├── dto.rs          # Create<X> / Update<X> request bodies (+ any query structs)
├── repository.rs   # all SQL, nothing else
├── service.rs      # validation + business rules; orchestrates repository calls
└── handlers.rs     # Axum handlers: extract inputs, call service, wrap for HTTP
```

Dependency direction is one-way: `handlers → service → repository`. Handlers
know no SQL; the repository knows no HTTP. A feature exposes **only** its
`router()`; submodules stay private (`mod dto;` not `pub mod dto;`).

---

## 5. Architecture & conventions (decisions with reasoning)

### Layering
`HTTP handler → application service → domain → repository → PostgreSQL`. No
business logic in handlers. This is my ASP.NET muscle memory; keep it.

### Error handling — `ApiError` in `src/error.rs`
- One enum: `NotFound | Validation | Conflict | Internal`.
- `impl IntoResponse for ApiError` renders a consistent envelope:
  `{"error":{"code","message"}}`. 5xx are logged server-side and their detail
  is **redacted** from the client response.
- `impl From<sqlx::Error> for ApiError` is what lets `?` convert repository
  errors as they propagate: **unique violation → 409 Conflict**, **foreign-key
  violation → 422 Validation**, everything else → 500 Internal.
- Mental model for me: `?` = explicit propagation that converts via `From`
  (vs. C# exceptions that propagate implicitly and convert nothing).
- **No `.unwrap()` / `.expect()` in request-handling paths.** Convert to
  `ApiError` instead.

### Fixed value-sets = Rust enum + `TEXT` + `CHECK`
- DB columns like `system_type`, `criticality`, `status`, `role`,
  `component_type` are `TEXT` with a `CHECK (... IN (...))` constraint — chosen
  over native Postgres `ENUM` types (rigid to alter) and lookup tables
  (overkill) because they're readable, extensible with a one-line migration, and
  map cleanly to Rust enums.
- Rust enums use `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` so the JSON
  boundary matches the DB tokens exactly, and an unknown value in a request body
  fails deserialization → **422 for free**.
- Each enum has `as_str()` (for SQL binds) and `from_db()` (for reads).
- **Row/domain split:** the repository fetches a `<X>Row` struct whose enum
  columns are `String`; `into_domain()` is the single place strings become typed
  enums (returns `Result`, maps unknown tokens to `ApiError::Internal` — never
  panics). The domain `<X>` struct is what the app and API responses use.
- **Exception:** `integration_types` is a real table (users must extend it at
  runtime), not a `TEXT + CHECK` set.

### DTO vs domain
`Create<X>` / `Update<X>` are request DTOs, distinct from the domain/response
struct: optional `slug` (derived from `name` via `slugify` when absent), field
defaults, no `id`/timestamps. `Update<X>` is all-`Option<T>` and the repository
does a partial update with `COALESCE($n, col)` — a `NULL` bind leaves a column
unchanged. **Caveat:** COALESCE can't set a nullable column back to `NULL`;
reach for `sqlx::QueryBuilder` if/when true nulling is needed.

### Schema conventions
- **UUID** primary keys, `DEFAULT gen_random_uuid()` (built into PG16, no
  extension).
- **`TIMESTAMPTZ`** everywhere (absolute UTC — never store local time).
- `created_at` / `updated_at` on mutable tables; a shared `set_updated_at()`
  trigger keeps `updated_at` fresh.
- **`JSONB metadata`** columns for extensible ad-hoc properties.
- Every top-level entity has `organization_id`. Single-org for now via
  `default_org_id()` = `Uuid::from_u128(1)`
  (`00000000-0000-0000-0000-000000000001`); this will come from the
  authenticated user once auth lands (later phase).

### Migrations
- `sqlx::migrate!("./migrations")` embeds the SQL into the binary and applies it
  at startup under an advisory lock. Because of this, **the API requires
  Postgres to be reachable at boot**.
- **Never edit an already-applied migration** (SQLx checksums them) — add a new
  numbered file.

### Queries
- Currently using the **runtime** `sqlx::query_as::<_, Row>(sql)` form
  (Dapper-style: validated at execution, not compile time) so the build needs no
  database. Phase 2c will move to the **compile-time** `query_as!` macros with a
  committed offline `.sqlx` cache — explain the trade-off (build-time DB
  dependency vs. typo-caught-at-compile) before switching.
- Useful pattern for optional filters, one statement, no dynamic SQL:
  `WHERE ($1::uuid IS NULL OR col = $1)` with a bound `Option<Uuid>`.

### Testing
`#[sqlx::test]` (needs the `macros` feature) creates a fresh, isolated,
migrated database per test and drops it after — like test containers, built in.
It reads `DATABASE_URL` from the environment at test time.

---

## 6. Local dev environment (Windows / PowerShell)

- **PowerShell gotchas:** use `curl.exe` (plain `curl` is an alias for
  `Invoke-WebRequest`); Unix line-continuation `\` becomes backtick `` ` ``.
- **Postgres runs in Docker on host port 5433**, not 5432 — 5432 is taken by
  another project's (Cookly's) native Postgres on this machine. The compose DB
  and the `.env` `DATABASE_URL` both use 5433.
- **`DATABASE_URL`** (local `cargo run`, via `.env`):
  `postgres://postgres:postgres@localhost:5433/integration_observability`.
  Inside the compose network the API instead reaches the DB by service name:
  `...@postgres:5432/...`.
- **Fast backend loop:** `docker compose up postgres` + `cargo run` from
  `apps/api` (recompiles in seconds). Running the API in Docker means an image
  rebuild per change — avoid that while actively writing Rust.
- **Frontend:** `npm run dev` in `apps/web` (Vite on :5173). Vite proxies
  `/api/*` → `http://localhost:8080`, so there's no CORS in dev; the frontend
  calls relative `/api/v1/...` paths.
- Config is read once at startup — **restart the process after changing an env
  var** (the lazy DB pool retries the connection each request but never re-reads
  config).

---

## 7. Do-not / gotchas

- **Do NOT add version pins for `home`, `base64ct`, `hashbrown`, `indexmap`, or
  `getrandom` to `Cargo.toml`.** If you ever see those, they were workarounds
  for an old sandbox toolchain and must not exist in this repo — it builds on
  current stable Rust (rustup), edition 2021.
- The `sqlx` dependency uses `default-features = false`, so **you own the whole
  feature list**. Current set:
  `["runtime-tokio","postgres","uuid","chrono","json","migrate","macros"]`.
  A missing feature typically shows up as a confusing "trait not implemented"
  far from the real cause (that's exactly how the missing `macros` bug
  surfaced).
- Axum 0.8 path params use braces: `/systems/{id}`, not `:id`.
- Keep company-specific services (Pagero/Kivra/etc.) out of the domain model —
  they're just rows with `system_type = 'EXTERNAL_SERVICE'`.
- Don't reintroduce a Cargo workspace until there's a second Rust crate.

---

## 8. Working rhythm with Claude Code

Prefer scoped tasks over "finish the phase." A good first task: implement the
`integrations` feature per §3/§5, extract `slugify` into `crate::util`, compile
and run tests, and explain the new Rust as you go. Then pause so I can review.
I'll often take a file or diff you produce over to a separate chat to talk
through it — that's expected; this repo is the source of truth.
