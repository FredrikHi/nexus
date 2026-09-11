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

const baseURL = process.env.AUTH_BASE_URL ?? "http://localhost:3010";

/**
 * Google is configured only when credentials are present, so a self-hoster who
 * only wants email and password does not have to register an OAuth client.
 */
const googleId = optional("GOOGLE_CLIENT_ID");
const googleSecret = optional("GOOGLE_CLIENT_SECRET");

export const auth = betterAuth({
  // Its own database. The Rust API never reads these tables: it trusts signed
  // tokens instead, which is what keeps identity swappable.
  database: new Pool({ connectionString: required("AUTH_DATABASE_URL") }),

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

  // The browser talks to the API on a different origin in development.
  trustedOrigins: (process.env.AUTH_TRUSTED_ORIGINS ?? "http://localhost:5173")
    .split(",")
    .map((o) => o.trim())
    .filter(Boolean),

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
