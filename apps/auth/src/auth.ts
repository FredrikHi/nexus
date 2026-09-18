import { betterAuth } from "better-auth";
import { jwt } from "better-auth/plugins";
import { Pool } from "pg";

/**
 * Reads a required setting, failing loudly at startup rather than producing a
 * service that appears healthy and rejects every sign-in.
 */
function required(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`${name} must be set`);
  }
  return value;
}

function optional(name: string): string | undefined {
  const value = process.env[name];
  return value && value.length > 0 ? value : undefined;
}

/**
 * Schema names are sent to Postgres as connection startup options, which are
 * plain text with no placeholders. Validate rather than escape.
 */
function identifier(name: string): string {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
    throw new Error(`AUTH_SCHEMA must be a plain identifier, got "${name}"`);
  }
  return name;
}

/**
 * Identity lives in its own schema of the same database the API uses.
 *
 * A schema rather than a second database because `CREATE SCHEMA` is available
 * to any owner, while `CREATE DATABASE` needs a privilege that several managed
 * Postgres providers never hand out. The boundary is still real: `auth.user`
 * and `public.users` cannot collide, and access to one can be revoked without
 * touching the other.
 *
 * Setting AUTH_DATABASE_URL puts identity in a separate database instead. You
 * then have to create that database yourself.
 */
const separateDatabase = optional("AUTH_DATABASE_URL");
export const authDatabaseUrl = separateDatabase ?? required("DATABASE_URL");
export const authSchema = identifier(
  process.env.AUTH_SCHEMA ?? (separateDatabase ? "public" : "auth"),
);

/**
 * The one public origin the whole product is served from. nginx puts the API
 * and this service behind it, so there is a single domain to configure and a
 * single origin the browser ever sees.
 */
export const appUrl = (process.env.APP_URL ?? "http://localhost:5173").replace(/[/]+$/, "");
const baseURL = process.env.AUTH_BASE_URL ?? appUrl;

/**
 * Exported so startup can print them. A mismatch between APP_URL and the
 * address the browser actually uses rejects every sign-in with a bare
 * "Invalid origin", which says nothing about what would have been accepted.
 */
export const trustedOrigins = [
  appUrl,
  ...(process.env.AUTH_TRUSTED_ORIGINS ?? "")
    .split(",")
    .map((o) => o.trim())
    .filter(Boolean),
];

/**
 * Google is configured only when credentials are present, so a self-hoster who
 * only wants email and password does not have to register an OAuth client.
 */
const googleId = optional("GOOGLE_CLIENT_ID");
const googleSecret = optional("GOOGLE_CLIENT_SECRET");

/**
 * Exported so a short-lived process, the migration, can close it when done.
 * Left open, its connections are cut when the process exits and Postgres logs
 * each one as "forcibly closed" on every start.
 */
export const authPool = new Pool({
  connectionString: authDatabaseUrl,
  // Scopes every statement to the schema above, including the tables the
  // migrator creates. pg_catalog is always searched implicitly, so the
  // built-ins still resolve.
  options: `-c search_path=${authSchema}`,
});

export const auth = betterAuth({
  // The Rust API never reads these tables: it trusts signed tokens instead,
  // which is what keeps identity swappable.
  database: authPool,

  baseURL,
  secret: required("AUTH_SECRET"),

  emailAndPassword: {
    enabled: true,
    // No mail sender is wired up yet, so requiring verification would lock
    // every new account out. Turn this on with an email provider.
    requireEmailVerification: false,
  },

  socialProviders:
    googleId && googleSecret
      ? { google: { clientId: googleId, clientSecret: googleSecret } }
      : {},

  // APP_URL is always trusted, since that is where the app is served from.
  // AUTH_TRUSTED_ORIGINS adds to it, for a second domain or a native client.
  trustedOrigins,

  plugins: [
    jwt({
      jwt: {
        // Must match AUTH_ISSUER and AUTH_AUDIENCE in the Rust API: a valid
        // signature is not enough if the token was minted for something else.
        issuer: baseURL,
        audience: process.env.AUTH_AUDIENCE ?? "integration-observability-api",
        // Short on purpose. Validation is local and stateless, so revoking a
        // session cannot take effect until the token expires; this bounds it.
        expirationTime: process.env.AUTH_TOKEN_TTL ?? "15m",
        definePayload: ({ user }) => ({
          sub: user.id,
          email: user.email,
          name: user.name,
          picture: user.image ?? undefined,
        }),
      },
    }),
  ],
});
