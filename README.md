<h1 align="center">Nexus</h1>

<p align="center">
  Open-source, self-hostable observability for the calls <em>between</em> your systems.
</p>

<p align="center">
  <a href="https://nexus-6s2.pages.dev">nexus-6s2.pages.dev</a>
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

On a Windows server with no Docker, `nexus.exe` runs the same stack as a single
Windows service instead: see
[Windows Server, without Docker](#windows-server-without-docker).

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

### Windows Server, without Docker

`nexus.exe` runs the whole stack as a single Windows service: PostgreSQL, the
API, the auth service, and the web app in front of them. Node.js 22 or newer is
the only thing the server needs installed; everything else travels in the
folder. Use this when the machine you have is a Windows server and a Linux VM
or Docker Desktop is not on the table.

There is no published Windows download yet, so the folder is assembled once on
a machine that has Rust and Node, and copied to the server:

```powershell
git clone https://github.com/FredrikHillbert/nexus
cd nexus
.\apps\nexus\scripts\assemble.ps1 -Destination C:\Nexus
```

That builds `nexus.exe`, the API, the web app and the auth bundles, downloads a
pinned PostgreSQL 18 with its checksum checked, and lays them out as
`nexus.exe` with `bin\`, `web\`, `auth\` and `pgsql\` beside it. Copy that
folder to the server, somewhere every account can read — `C:\Program
Files\Nexus` or `C:\Nexus`. Never inside a user profile: the service runs as
its own account and cannot read another account's files, and `install` refuses
such a path rather than letting it fail later.

Then, from an **elevated** PowerShell on the server:

```powershell
C:\Nexus\nexus.exe install --url https://nexus.example.com
```

`--url` is the address people will type. It is also the token issuer and the
only origin sign-in is accepted from, so it has to be the public one, not
`localhost`.

Installing registers the service, generates `AUTH_SECRET`, writes the settings
to `%ProgramData%\Nexus\nexus.env` and locks that folder to administrators and
the service alone, then starts it and waits until Nexus answers. The first
start creates the database and takes a minute or so. Installing again over an
existing `nexus.env` — after an `uninstall`, which is what a second `install`
asks for — keeps the secret that is already there. Replacing it would sign
everyone out and make the auth service's stored signing keys unreadable.

| | |
| --- | --- |
| `C:\ProgramData\Nexus\nexus.env` | Settings. `KEY=value`, the same names as the environment variables. |
| `C:\ProgramData\Nexus\postgres\` | The database. Survives an upgrade that replaces the program folder. |
| `C:\ProgramData\Nexus\logs\` | One file per day, 14 kept. The first place to look when something is wrong. |
| `NT SERVICE\Nexus` | The virtual account it runs as. No password to manage, and none of an administrator's rights. |

The service runs as an unprivileged virtual account deliberately: PostgreSQL
refuses to run with administrator rights at all. That is also why `nexus run`
fails in an elevated console while `nexus install` requires one — install talks
to the service manager, running does not.

#### Putting it on the network

The front door listens on `127.0.0.1:5173`, reachable from the server itself
only, because anyone who can reach Nexus can create an account. There are two
ways to open it up.

**Behind IIS or another reverse proxy**, which is the one to prefer: terminate
TLS there and forward to `http://127.0.0.1:5173`. An `X-Forwarded-Proto` that
arrives is passed on rather than overwritten, so the auth service knows the
browser used https and marks its cookies Secure. `APP_URL` stays the https
address.

**Directly**, when something else already handles TLS or the server is on a
trusted network only: set `LISTEN_ADDR` to `0.0.0.0:8080` in `nexus.env`, make
`APP_URL` the address people will actually type, restart, and open the port.

```powershell
notepad C:\ProgramData\Nexus\nexus.env    # elevated: only administrators can read it
Restart-Service Nexus
New-NetFirewallRule -DisplayName "Nexus" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow
```

Without TLS in front, passwords and session cookies cross the network in the
clear. `APP_URL` has to match what people type either way, port included.

#### Running it

```powershell
Start-Service Nexus
Stop-Service Nexus          # PostgreSQL checkpoints first; allow up to a minute
Get-Content "C:\ProgramData\Nexus\logs\nexus.$(Get-Date -Format yyyy-MM-dd).log" -Tail 50 -Wait
```

To upgrade: assemble the new version, `Stop-Service Nexus`, replace the program
folder, `Start-Service Nexus`. Settings and data live in `%ProgramData%` and are
untouched. `nexus uninstall` removes the service and keeps both; delete
`C:\ProgramData\Nexus` to remove them too.

To try it in a console before installing anything, from a **non-elevated**
prompt:

```powershell
$env:AUTH_SECRET = -join ((1..32) | ForEach-Object { "{0:x2}" -f (Get-Random -Max 256) })
C:\Nexus\nexus.exe run
```

It runs until Ctrl+C, on `http://localhost:5173`, with its database in
`%ProgramData%\Nexus`.

#### Settings on Windows

The shared settings below apply, with these additions. `APP_URL` and
`AUTH_SECRET` are written for you by `install`; `POSTGRES_PASSWORD`,
`IMAGE_TAG` and `WEB_PORT` belong to the Docker stack only.

They are read from `nexus.env` *instead of* the environment, never a mix of the
two. A service runs with the machine's environment, and a server that hosts
other applications may well have a `DATABASE_URL` or an `APP_URL` set for one
of them; mixing would quietly point Nexus at someone else's database.

| Variable | Default | |
| --- | --- | --- |
| `LISTEN_ADDR` | `127.0.0.1:5173` | Where the front door listens. `--listen` writes this. |
| `NODE_EXE` | `node.exe` on PATH | Resolved at install time and written down, so a PATH change later cannot move it. |
| `DATABASE_URL` | unset | Set it to use a PostgreSQL server you already run; unset, Nexus runs its own. `--database-url` writes it. |
| `DATA_DIR` | beside `nexus.env` | Where the database and its password file go. |
| `API_ADDR` / `AUTH_ADDR` / `POSTGRES_ADDR` | `127.0.0.1:18080` / `:13010` / `:15432` | Loopback only, and refused otherwise: nothing behind the front door should be reachable on its own. Change them if a port is taken. |
| `API_RUST_LOG` | `integration_api=info,warn` | The API's log filter. Not `RUST_LOG`, which is `nexus.exe`'s own and would otherwise be inherited by the API and filter out all of its logs. |

Node.js has to be installed for all users. `nvm` for Windows puts it inside the
profile of whoever ran it, which the service account cannot read; `install`
refuses that too, with the way out.

#### Backing up the built-in database

```powershell
$env:PGPASSWORD = (Get-Content C:\ProgramData\Nexus\postgres.password -Raw).Trim()
C:\Nexus\pgsql\bin\pg_dump.exe -h 127.0.0.1 -p 15432 -U postgres -f backup.sql integration_observability
```

From an elevated console: the password file is readable by administrators and
the service only. It is generated on first start and is the only way into that
database — a backup of `C:\ProgramData\Nexus` that omits it restores nothing.

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

Two clients, both of which never throw, never block a request, and drop
telemetry rather than growing without bound:

| | |
| --- | --- |
| [Node](clients/node) | One file, no dependencies. |
| [.NET](clients/dotnet) | `dotnet add package Nexus.Observability`. One line per `HttpClient` reports everything it does. |

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

`apps/nexus` is the Windows host: a second Rust crate, with its own
`cargo test` that needs no database and runs on Windows in CI.

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

Work happens on a branch and lands through a pull request. Seven checks have
to be green: the two Rust suites, the API and the Windows host, with formatting
and clippy as errors; an install-use-stop-uninstall of the Windows service on a
real machine; the web build and lint; a typecheck of the auth service and the
Node client; and a build and pack of the .NET client.

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
commits, and publishes images tagged with the new version alongside `latest`,
plus `Nexus.Observability` on nuget.org carrying the same version. A merge that
releases nothing publishes nothing, so `latest` always points at the most
recent release rather than the most recent commit.

## License

The platform is [GNU AGPL v3](LICENSE). You may run, modify and redistribute it
freely. The one obligation that matters: if you offer it to others over a
network as a service, you have to offer them the source of your modified
version too. That is the whole point of the Affero clause, and it is why this
licence rather than MIT.

The clients in [clients/](clients) are [MIT](clients/dotnet/LICENSE). They are
linked into the applications being watched, and copyleft reaching in there
would mean the price of recording how long a call took is publishing the
service that made it. The Affero clause is aimed at whoever runs Nexus, not at
whoever is measured by it.
