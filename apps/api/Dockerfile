# ---- build stage ----
FROM rust:1-bookworm AS builder
WORKDIR /app

# Validate every sqlx query against the committed .sqlx cache rather than a
# live database. Without this the macros try to connect and the build fails,
# because DATABASE_URL being set makes sqlx prefer a real connection over the
# cache. Regenerate the cache with `cargo sqlx prepare` after changing a query.
ENV SQLX_OFFLINE=true

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
# migrate!() reads these at COMPILE time and embeds them in the binary, so the
# directory has to exist in the build stage even though nothing opens it later.
COPY migrations ./migrations
COPY .sqlx ./.sqlx
RUN cargo build --release

# ---- runtime stage ----
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/integration-api /usr/local/bin/integration-api
ENV HOST=0.0.0.0 PORT=8080
EXPOSE 8080
CMD ["integration-api"]
