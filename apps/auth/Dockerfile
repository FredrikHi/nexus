# Integration Observability — auth service (Better Auth on Node)
#
# The service runs TypeScript directly through tsx, so there is no compile
# step; tsx is a production dependency for that reason. What the stages buy
# us is a final image with only the production dependency tree in it.

# ==========================================
# Stage 1: Base image
# ==========================================
# Debian 12 "bookworm", NOT bullseye: Debian 11 left LTS on 2026-08-31 and
# `apt-get update` against deb.debian.org now fails there.
FROM node:22-bookworm-slim AS base
WORKDIR /app
ENV NODE_ENV=production

# openssl + ca-certificates for TLS to Postgres and to the OAuth providers.
# wget is what Coolify's default container healthcheck invokes; curl is for
# the HEALTHCHECK below.
RUN apt-get update \
    && apt-get install -y --no-install-recommends openssl ca-certificates curl wget \
    && rm -rf /var/lib/apt/lists/*

# ==========================================
# Stage 2: Production dependencies
# ==========================================
FROM base AS production-deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --omit=dev

# ==========================================
# Stage 3: Final production image
# ==========================================
FROM base AS production
WORKDIR /app

# Created before the COPY so --chown can set ownership in one pass, instead of
# a slow recursive chown over node_modules afterwards.
RUN groupadd -r nodejs && useradd -m -r -g nodejs auth
ENV HOME=/home/auth

COPY --from=production-deps --chown=auth:nodejs /app/node_modules /app/node_modules
COPY --chown=auth:nodejs package.json ./package.json
COPY --chown=auth:nodejs tsconfig.json ./tsconfig.json
COPY --chown=auth:nodejs src ./src
COPY --chown=auth:nodejs docker-entrypoint.sh ./docker-entrypoint.sh

USER auth

ENV PORT=3010
EXPOSE 3010

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:${PORT}/health || exit 1

# The entrypoint applies the Better Auth schema before the CMD starts, so a
# deploy needs no manual migration step. Set AUTH_SKIP_MIGRATIONS=true to skip.
ENTRYPOINT ["sh", "docker-entrypoint.sh"]

# Configuration comes from the environment in a container, so the .env file is
# optional rather than required.
CMD ["node", "--env-file-if-exists=.env", "--import", "tsx", "src/index.ts"]
