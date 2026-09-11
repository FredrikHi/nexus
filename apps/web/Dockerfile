# Integration Observability — web frontend (React + Vite, served by nginx)
#
# The build produces static files; nginx serves them and reverse-proxies the
# two backends so the browser only ever sees one origin. That is what keeps
# the app free of CORS and third-party-cookie problems in production, exactly
# as the Vite dev proxy does in development.

# ==========================================
# Stage 1: Build the static bundle
# ==========================================
# NODE_ENV is deliberately NOT set to production here: `npm ci` would then skip
# devDependencies, and vite/typescript/tailwind all live there.
FROM node:22-bookworm-slim AS build
WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .
RUN npm run build

# ==========================================
# Stage 2: Serve
# ==========================================
FROM nginx:1.27-alpine AS production

# Where nginx forwards each API prefix. Defaults match the compose service
# names; override them in Coolify if the services are reachable elsewhere.
# NGINX_ENVSUBST_FILTER limits substitution to these names, so nginx's
# own $uri/$host variables survive the entrypoint's envsubst pass.
ENV API_UPSTREAM=api:8080 \
    AUTH_UPSTREAM=auth:3010 \
    NGINX_ENTRYPOINT_LOCAL_RESOLVERS=1 \
    NGINX_ENVSUBST_FILTER="^(API_UPSTREAM|AUTH_UPSTREAM|NGINX_LOCAL_RESOLVERS)$"

# Rendered into /etc/nginx/conf.d/default.conf at container start.
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 80

# busybox wget — the alpine image has no curl, and this is the same binary
# Coolify's own default healthcheck reaches for.
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD wget -q --spider http://127.0.0.1/healthz || exit 1
