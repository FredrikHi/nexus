#!/bin/sh
# Container entrypoint: bring the auth schema up to date, then hand over to the
# CMD. Deliberately mirrors the Rust API, which runs its sqlx migrations at
# startup, so a deploy never needs a manual step against the database.
#
# Both take an advisory lock, so two instances starting at once is safe.
set -e

if [ "${AUTH_SKIP_MIGRATIONS:-false}" = "true" ]; then
  echo "entrypoint: AUTH_SKIP_MIGRATIONS=true, leaving the schema alone"
else
  echo "entrypoint: applying auth migrations"
  node --import tsx src/migrate.ts
fi

# exec: replace this shell with the service, so it becomes PID 1 and receives
# SIGTERM from Docker directly. Without it the shell would swallow the signal
# and the container would be killed on the stop timeout instead of shutting
# down cleanly.
exec "$@"
