import { createServer } from "node:http";
import { toNodeHandler } from "better-auth/node";
import { auth, appUrl, trustedOrigins } from "./auth.js";

const port = Number(process.env.PORT ?? 3010);
const handler = toNodeHandler(auth);

const server = createServer((req, res) => {
  // Everything under /api/auth is Better Auth's: sign-up, sign-in, the OAuth
  // callbacks, /token and the JWKS the Rust API fetches.
  if (req.url?.startsWith("/api/auth")) {
    void handler(req, res);
    return;
  }

  if (req.url === "/health") {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ status: "ok" }));
    return;
  }

  res.writeHead(404, { "content-type": "application/json" });
  res.end(JSON.stringify({ error: { code: "NOT_FOUND", message: "no such route" } }));
});

server.listen(port, () => {
  console.log(`auth service listening on http://0.0.0.0:${port}`);
  // Printed because getting APP_URL wrong is the most likely first-run
  // mistake, and the 403 it causes does not say what was expected.
  console.log(`  app url:         ${appUrl}`);
  console.log(`  trusted origins: ${trustedOrigins.join(", ")}`);
  console.log("  a sign-in from any other origin is refused as INVALID_ORIGIN");
});
