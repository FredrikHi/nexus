# Integration Observability Platform

A single place to see the systems you run, the integrations between them, and
what is actually happening across them: which calls are failing, how slowly,
and since when.

It is built for the case where the interesting failures happen *between*
services rather than inside one. Your application's own logs will tell you a
request was slow. They will not tell you that the slowness started when a
third party began rate-limiting you, that it has been going on for two hours,
or that it only affects one of the six places you call them from.

Open source and self-hostable. One command, one domain, three settings.

## The idea

Seven things, each built on the one before:

| | |
| --- | --- |
| **Systems** | The things you run or depend on. Your API, your database, Stripe. |
| **Components** | The parts inside them. A service, a table, an endpoint. |
| **Integrations** | An edge from one component to another. This is the unit everything else is measured against. |
| **Telemetry** | One record per call across an edge: outcome, duration, trace. |
| **Traces** | The calls one request caused, correlated. |
| **Health** | A status per integration, re-derived on a timer from recent telemetry. |
| **Incidents** | Opened and resolved automatically when health changes, so an outage has a start, an end, and a history. |

Nothing about any particular vendor is built in. Stripe, OpenAI and your own
database are all just rows.

## Deploy it

You need Docker, a domain pointed at the host, and about 1 GB of RAM.

```bash
git clone <this repository>
cd integration-observability
cp .env.example .env
```

Set three things in `.env`:

```bash
APP_URL=https://icc.example.com        # where a browser reaches it, no trailing slash
AUTH_SECRET=                           # openssl rand -hex 32
POSTGRES_PASSWORD=                     # anything long
```

Then:

```bash
docker compose up -d
```

Open `APP_URL`, create an account, create an organization. The first account
to sign up is not special: this is multi-tenant from the first row, and every
organization is isolated from every other.

### On Coolify

Same compose file, no changes.

1. New Resource, Docker Compose, pointed at your fork of this repository.
2. Set `APP_URL`, `AUTH_SECRET` and `POSTGRES_PASSWORD` in the environment tab.
3. Give the **web** service your domain, on port 80.

If Coolify's proxy routes to the container directly, the host port the compose
file publishes is redundant. `WEB_PORT` controls it, and you can drop that
`ports:` entry entirely once the domain is attached.

Only `web` gets a public address. It serves the app and reverse-proxies
`/api/v1` and `/api/auth` to the other two over the private network, which is
why there is one domain rather than three, and why the browser never meets a
cross-origin request or a third-party cookie.

Leave the API, the auth service and Postgres unpublished. They are reachable
by service name inside the network and have no business being reachable from
anywhere else.

## Configuration

Three settings matter. Everything else has a working default and exists so you
can change it, not so you must.

| Variable | Default | |
| --- | --- | --- |
| `APP_URL` | `http://localhost:5173` | The one public origin. Also the token issuer, so the API and the auth service must agree, which they do by both reading this. |
| `AUTH_SECRET` | none, required | Signs sessions and tokens. Changing it signs everyone out. |
| `POSTGRES_PASSWORD` | `postgres` | Change it. |
| `GOOGLE_CLIENT_ID` / `_SECRET` | empty | Google sign-in appears only when both are set. Redirect URI is `<APP_URL>/api/auth/callback/google`. |
| `TELEMETRY_RETENTION_DAYS` | `90` | Older telemetry has its whole monthly partition dropped. |
| `HEALTH_EVAL_INTERVAL_SECONDS` | `60` | How often health is re-derived. |
| `RUST_LOG` | `integration_api=info,warn` | `integration_api=debug` logs every request. Useful locally, far too loud in production. |
| `WEB_PORT` | `5173` | Host port the web container publishes on. |
| `AUTH_TRUSTED_ORIGINS` | empty | Extra origins allowed to sign in, comma separated. `APP_URL` is always trusted; this is for a second domain or a native client. |

### One database

Identity keeps its own `auth` schema inside the same database the rest of the
product uses, created for you on first boot. A schema rather than a second
database because `CREATE SCHEMA` is available to any owner, while
`CREATE DATABASE` needs a privilege several managed Postgres providers never
grant. The boundary is still real: `auth.user` and `public.users` cannot
collide, and the API never reads across it. It verifies signed tokens instead,
which is what lets identity be re-pointed at something else later.

If you would rather have a separate database, set `AUTH_DATABASE_URL` and
create that database yourself.

## Send it data

The platform is empty until something reports to it. See
[clients/node](clients/node) for the Node client: one file, no dependencies,
and it cannot break or slow down the application it is watching.

Model your landscape as a file and apply it, rather than clicking it together
once:

```bash
node scripts/apply-landscape.mjs my-landscape.json \
  --url "$APP_URL" --token "$TOKEN" --org "$ORG_ID" \
  --emit src/lib/integrations.ts
```

Re-running only creates what is missing, so keep the file in version control
next to the code it describes. `examples/cookly.landscape.json` is a real one.

## Develop it

Run only what you are not editing:

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up postgres auth
cd apps/api && cargo run          # :8080
cd apps/web && npm run dev        # :5173, proxies /api to the two above
```

The override publishes Postgres on 5433 and the auth service on 3010, which
the production stack deliberately keeps off the host. `cargo run` recompiles
in seconds; running the API in Docker means an image rebuild per change.

Tests use `#[sqlx::test]`, which creates a fresh migrated database per test and
drops it afterwards:

```bash
cd apps/api && cargo test
```

## How it fits together

| Service | | |
| --- | --- | --- |
| `web` | React, served by nginx | The only public service. Proxies the two below. |
| `api` | Rust, Axum, SQLx | The domain. Applies its own migrations at boot. |
| `auth` | Node, Better Auth | Sign-in only. Applies its own schema at boot. |
| `postgres` | Postgres 16 | One database, two schemas. |

Both application services migrate themselves on start, under an advisory lock,
so a deploy never needs a manual step against the database and two instances
starting at once is safe.

## Backups

One database holds everything:

```bash
docker compose exec postgres pg_dump -U postgres integration_observability | gzip > backup.sql.gz
```

Telemetry costs roughly 580 bytes an event. A million events a month is around
580 MB a month, and the retention setting above is what bounds it.
