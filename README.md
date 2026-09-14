<h1 align="center">Integration Control Center</h1>

<p align="center">
  Open-source, self-hostable observability for the calls <em>between</em> your systems.
</p>

<p align="center">
  <a href="https://fredrikhillbert.github.io/nexus/">fredrikhillbert.github.io/nexus</a>
</p>

<p align="center">
  <img alt="License" src="https://img.shields.io/github/license/FredrikHillbert/nexus?style=for-the-badge&color=4c1">
  <img alt="Stars" src="https://img.shields.io/github/stars/FredrikHillbert/nexus?style=for-the-badge&logo=github&color=f5c518">
  <img alt="Latest release" src="https://img.shields.io/github/v/release/FredrikHillbert/nexus?style=for-the-badge&logo=github&color=6f42c1&sort=semver">
  <img alt="Stack" src="https://img.shields.io/badge/Rust-Axum-000000?style=for-the-badge&logo=rust">
  <img alt="Stack" src="https://img.shields.io/badge/React_19-TypeScript-3178c6?style=for-the-badge&logo=react">
  <img alt="Stack" src="https://img.shields.io/badge/PostgreSQL_18-316192?style=for-the-badge&logo=postgresql&logoColor=white">
</p>

## What is this?

A single place to see the systems you run, the integrations between them, and
what is actually happening across them: which calls are failing, how slowly,
and since when.

## Why?

Because the interesting failures happen between services, not inside one.

Your application logs will tell you a request was slow. They will not tell you
that the slowness began when a third party started rate-limiting you, that it
has been going on for two hours, or that it affects one of the six places you
call that third party from and none of the others.

Nothing about any particular vendor is built in. Stripe, OpenAI and your own
database are all just rows, so this describes your landscape rather than a
catalogue of supported services.

## Features

- **Nothing to model up front.** Name an integration in your code and the first
  event creates it. Describe the landscape deliberately in a JSON file instead
  when you would rather review it in a pull request.
- **Landscape as data.** Systems, the components inside them, and the
  integrations between them, all editable in the UI or applied from a file.
- **Telemetry that cannot hurt you.** The client queues in memory, flushes on a
  timer, drops rather than grows, and swallows its own errors.
- **Traces.** Every call one request caused, correlated automatically.
- **Health on a timer.** Re-derived from recent telemetry, so a status change
  is noticed while nobody is watching.
- **Incidents that open and close themselves,** giving an outage a start, an
  end, and a history.
- **Refusals are not failures.** A 429 is recorded as REJECTED, so a rate limit
  never makes a working dependency look broken.
- **Multi-tenant from the first row.** Organizations, roles, and per-tenant
  isolation enforced in every query.
- **Ingest keys pinned to an environment,** so a staging agent cannot write
  events claiming to be production.

## Self-hosting

The recommended way to run this is the prebuilt images:

```bash
docker pull ghcr.io/fredrikhillbert/nexus-web:latest
```

### Quick start

```bash
curl -O https://raw.githubusercontent.com/FredrikHillbert/nexus/master/docker-compose.production.yml

cat > .env <<EOF
APP_URL=https://icc.example.com
AUTH_SECRET=$(openssl rand -hex 32)
POSTGRES_PASSWORD=$(openssl rand -hex 16)
EOF

docker compose -f docker-compose.production.yml up -d
```

Nothing is built and nothing is cloned: three images are pulled, Postgres comes
up beside them, and both application services apply their own schema on first
boot. Open `APP_URL`, create an account, create an organization.

### Coolify

Public Repository, `https://github.com/FredrikHillbert/nexus`, Build Pack
**Docker Compose**, compose file `docker-compose.production.yml`. Give the
**web** service your domain on port 80, and set `APP_URL`, `AUTH_SECRET` and
`POSTGRES_PASSWORD`.

Use the *Application* resource type, not the *Docker Compose* service type.
The service type stores only the file you paste, with no repository beside it.
A private repository needs a GitHub App source connected first; a public one
needs only its URL.

Also set `WEB_PORT` to something nothing else on that host is using. The
compose file publishes a host port so a plain `docker compose up` on a bare
VPS works, but on a server already running other things the default collides
and the deploy fails with `Bind for 0.0.0.0:8080 failed: port is already
allocated`. Coolify's proxy reaches the container over the Docker network, so
that published port is only there for direct access and you can drop the
`ports:` entry once the domain is attached.

### Image visibility

Images published from a private repository are private, and a private image
fails the deploy with `denied` on the manifest. GHCR returns the same `denied`
whether an image is private or absent, so check both: that the **Publish
images** workflow actually ran, and that each of the three packages is set to
Public under your GitHub packages settings.

To keep them private instead, give the deployment host a pull credential:

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u <your-github-user> --password-stdin
```

with a token carrying `read:packages`.

### Settings

Three matter. Everything else has a working default.

| Variable | Default | |
| --- | --- | --- |
| `APP_URL` | none, required | The one public origin. Also the token issuer, so the API and the auth service agree by both reading this. No trailing slash. |
| `AUTH_SECRET` | none, required | Signs sessions and tokens. Changing it signs everyone out. |
| `POSTGRES_PASSWORD` | none, required | |
| `IMAGE_TAG` | `latest` | Pin a release instead of tracking latest. |
| `WEB_PORT` | `8080` | Host port the web container publishes on. Change it if something already holds that port. |
| `GOOGLE_CLIENT_ID` / `_SECRET` | empty | Google sign-in appears only when both are set. Redirect URI is `<APP_URL>/api/auth/callback/google`. |
| `TELEMETRY_RETENTION_DAYS` | `90` | Older telemetry has its whole monthly partition dropped. |
| `HEALTH_EVAL_INTERVAL_SECONDS` | `60` | How often health is re-derived. |
| `RUST_LOG` | `integration_api=info,warn` | `integration_api=debug` logs every request. Far too loud in production. |
| `AUTH_TRUSTED_ORIGINS` | empty | Extra sign-in origins, comma separated. `APP_URL` is always trusted. |

Only the web service needs a public address. It serves the app and
reverse-proxies `/api/v1` and `/api/auth` to the other two over the private
network, which is why there is one domain rather than three and why the browser
never meets a cross-origin request.

Identity keeps its own `auth` schema inside the same database, created on first
boot. A schema rather than a second database because `CREATE SCHEMA` is
available to any owner, while `CREATE DATABASE` needs a privilege several
managed Postgres providers never grant. Set `AUTH_DATABASE_URL` if you would
rather separate them, and create that database yourself.

### Backups

One database holds everything:

```bash
docker compose exec postgres pg_dump -U postgres integration_observability | gzip > backup.sql.gz
```

Telemetry costs roughly 580 bytes an event. A million events a month is about
580 MB, and `TELEMETRY_RETENTION_DAYS` is what bounds it.

## Sending it data

Empty until something reports to it. Under **Administration, API keys**,
create one and leave *Create unknown integrations* ticked. The token is shown
once and stored only as a hash. Then in your application:

```ts
await observability.track("orders-to-stripe", () => stripe.charges.create(...))
```

That is the whole setup. The first event naming `orders-to-stripe` creates that
integration, so there is nothing to model up front and no generated file of
identifiers to keep in step with one particular database. The same build
reports to a laptop and to production, and the environment comes from the key
rather than the code.

Discovered integrations land under a placeholder system called **Unmapped**.
Wire them to their real endpoints in the UI when you are ready; renaming one
keeps its slug, so the code goes on working. Turn auto-create off once the
picture is complete, and a misspelled slug becomes a refusal again rather than
a new row.

The Node client is one file with no dependencies: see
[clients/node](clients/node). It never throws, never blocks a request, and
drops telemetry rather than growing without bound.

Prefer to describe the landscape deliberately and review it in a pull request?
Apply a file instead, and keep auto-create off:

```bash
node scripts/apply-landscape.mjs my-landscape.json   --url "$APP_URL" --token "$TOKEN" --org "$ORG_ID"
```

Re-running creates only what is missing. `examples/cookly.landscape.json` is a
real one.

## Development

Rust with Axum and SQLx, React 19 with TypeScript and Vite, Better Auth on
Node, Postgres 18.

```bash
docker compose -f docker-compose.yml -f docker-compose.dev.yml up postgres auth
cd apps/api && cargo run          # :8080
cd apps/web && npm run dev        # :5173, proxies /api to the two above
```

The override publishes Postgres on 5433 and the auth service on 3010, which
the production stack deliberately keeps off the host. `cargo run` recompiles in
seconds; running the API in Docker means an image rebuild per change.

```bash
cd apps/api && cargo test
```

Tests use `#[sqlx::test]`, which creates a fresh migrated database per test and
drops it afterwards.

| Service | | |
| --- | --- | --- |
| `web` | React, served by nginx | The only public service. Proxies the two below. |
| `api` | Rust, Axum, SQLx | The domain. Applies its own migrations at boot. |
| `auth` | Node, Better Auth | Sign-in only. Applies its own schema at boot. |
| `postgres` | Postgres 18 | One database, two schemas. |

Both application services migrate themselves on start under an advisory lock,
so a deploy never needs a manual step and two instances starting at once is
safe.

## Contributing

Work happens on a branch and lands through a pull request. Five checks have to
be green: the Rust suite with formatting and clippy as errors, the web build
and lint, and a typecheck of the auth service and the Node client.

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org),
because the version is computed from them rather than chosen by hand:

| Prefix | Effect |
| --- | --- |
| `fix:` | Patch release, 1.2.3 to 1.2.4 |
| `feat:` | Minor release, 1.2.3 to 1.3.0 |
| `feat!:` or a `BREAKING CHANGE:` footer | Major release, 1.2.3 to 2.0.0 |
| `chore:`, `docs:`, `refactor:`, `test:` | No release |

CI checks the pull request title rather than each commit, because a squash
merge uses the title as the commit message. That title is what decides the
next version.

Merging to the default branch tags the release, writes the notes from the
commits, and publishes images tagged with the new version alongside `latest`.
A merge that releases nothing publishes nothing, so `latest` always points at
the most recent release rather than the most recent commit.

## License

[GNU AGPL v3](LICENSE).

You may run, modify and redistribute this freely. The one obligation that
matters: if you offer it to others over a network as a service, you have to
offer them the source of your modified version too. That is the whole point of
the Affero clause, and it is why this licence rather than MIT.
