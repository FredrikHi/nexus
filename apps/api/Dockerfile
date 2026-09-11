# Integration Observability — Rust API
#
# Two stages: a fat Rust toolchain image compiles the binary, a slim Debian
# image runs it. The final image carries no compiler and no source.
#
# Debian 12 "bookworm", NOT bullseye: Debian 11 left LTS on 2026-08-31, its
# packages moved to archive.debian.org, and `apt-get update` now fails there.

# ==========================================
# Stage 1: Build
# ==========================================
FROM rust:1-bookworm AS builder
WORKDIR /app

# Validate every sqlx query against the committed .sqlx cache rather than a
# live database. Without this the macros try to connect and the build fails,
# because DATABASE_URL being set makes sqlx prefer a real connection over the
# cache. Regenerate the cache with `cargo sqlx prepare` after changing a query.
ENV SQLX_OFFLINE=true

# Compile the dependency tree on its own first, against a placeholder main.rs.
# Docker caches that layer, so editing our own source recompiles only this
# crate instead of the ~400 crates underneath it.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

# Now the real source. migrate!() and the sqlx macros read migrations/ and
# .sqlx at COMPILE time and embed the result in the binary, so both have to be
# present here even though nothing opens them at runtime.
COPY src ./src
COPY migrations ./migrations
COPY .sqlx ./.sqlx
# The placeholder above left a cached fingerprint; touching forces a rebuild.
RUN touch src/main.rs && cargo build --release --locked

# ==========================================
# Stage 2: Runtime
# ==========================================
FROM debian:bookworm-slim AS production
WORKDIR /app

# ca-certificates: the API fetches the auth service's JWKS over HTTPS in
# production, and rustls has no trust store of its own to fall back on.
# wget is what Coolify's default container healthcheck invokes; curl is for
# the HEALTHCHECK below and for debugging inside the container.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl wget \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged user. Created before the COPY so --chown can set
# ownership in one pass.
RUN groupadd -r app && useradd -r -g app -m -d /home/app app
ENV HOME=/home/app

COPY --from=builder --chown=app:app /app/target/release/integration-api /usr/local/bin/integration-api

USER app

ENV HOST=0.0.0.0 \
    PORT=8080 \
    RUST_LOG=integration_api=info,tower_http=info,info

EXPOSE 8080

# Liveness only — readiness pings the database, and a database blip should not
# make Docker restart a healthy API.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:${PORT}/api/v1/health || exit 1

CMD ["integration-api"]
