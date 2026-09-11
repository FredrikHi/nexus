/**
 * Creates the auth schema if it is missing, applies the Better Auth schema
 * into it, then exits.
 *
 * This is the same work `@better-auth/cli migrate` does, called directly
 * instead. Two reasons: the CLI is published on its own release train (1.4.x
 * while the library is on 1.6.x), and a container should not be downloading a
 * floating `npx` package at boot. `getMigrations` ships inside the library we
 * already depend on, so the schema and the code that serves it are always the
 * same version.
 *
 * Idempotent: it diffs the live schema against the one the configured plugins
 * need, and does nothing when they already agree.
 */
import { getMigrations } from "better-auth/db/migration";
import { Pool } from "pg";
import { auth, authDatabaseUrl, authSchema } from "./auth.js";

// An arbitrary but fixed key. Two instances starting at once must not both try
// to create the same tables, so the loser waits here and then finds nothing to
// do. The Rust API's sqlx migrator takes an advisory lock for the same reason.
const MIGRATION_LOCK_KEY = 4_919_3010;

async function main(): Promise<void> {
  const pool = new Pool({ connectionString: authDatabaseUrl });
  const lock = await pool.connect();

  try {
    await lock.query("SELECT pg_advisory_lock($1)", [MIGRATION_LOCK_KEY]);

    // Under the lock, so two instances starting together cannot race here.
    // The schema name is validated as an identifier in auth.ts; CREATE SCHEMA
    // takes no parameters, so it has to be interpolated.
    await lock.query(`CREATE SCHEMA IF NOT EXISTS "${authSchema}"`);

    const { toBeCreated, toBeAdded, runMigrations } = await getMigrations(auth.options);

    if (toBeCreated.length === 0 && toBeAdded.length === 0) {
      console.log("auth schema already up to date");
      return;
    }

    const created = toBeCreated.map((t) => t.table).join(", ");
    const altered = toBeAdded.map((t) => t.table).join(", ");
    console.log(
      `applying auth migrations to schema "${authSchema}": ` +
        `create [${created || "none"}], alter [${altered || "none"}]`,
    );

    await runMigrations();
    console.log("auth migrations applied");
  } finally {
    await lock.query("SELECT pg_advisory_unlock($1)", [MIGRATION_LOCK_KEY]);
    lock.release();
    await pool.end();
  }
}

main()
  .then(() => {
    // auth.ts opened its own connection pool at import time and nothing closes
    // it, so the event loop would otherwise keep this process alive forever.
    process.exit(0);
  })
  .catch((error: unknown) => {
    // Exit non-zero so the container entrypoint stops rather than starting a
    // service that will fail every request against a half-built schema.
    console.error("auth migrations failed:", error);
    process.exit(1);
  });
