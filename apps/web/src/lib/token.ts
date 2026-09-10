// The session lives in an http-only cookie the browser sends automatically.
// The Rust API does not read cookies: it wants a bearer token, which the auth
// service mints from that session and which expires in about 15 minutes.
//
// The token is kept in memory only, never in localStorage. A stolen token is
// usable until it expires, and localStorage hands it to any injected script.

interface CachedToken {
  token: string
  /** Epoch milliseconds. */
  expiresAt: number
}

let cached: CachedToken | null = null
let inFlight: Promise<string | null> | null = null

/** Renew this long before expiry so a request never carries a token that
 *  expires mid-flight. */
const RENEW_MARGIN_MS = 60_000

function decodeExpiry(token: string): number {
  try {
    const payload = token.split('.')[1]
    const json = atob(payload.replace(/-/g, '+').replace(/_/g, '/'))
    const exp = JSON.parse(json).exp
    return typeof exp === 'number' ? exp * 1000 : 0
  } catch {
    // Unreadable expiry means treat it as already expired rather than trusting it.
    return 0
  }
}

async function fetchToken(): Promise<string | null> {
  const res = await fetch('/api/auth/token', { credentials: 'include' })
  if (!res.ok) return null

  const body = (await res.json()) as { token?: string }
  if (!body.token) return null

  cached = { token: body.token, expiresAt: decodeExpiry(body.token) }
  return body.token
}

/**
 * A valid bearer token, or null when nobody is signed in.
 *
 * Concurrent callers share one request: without that, a page loading six
 * queries at once would mint six tokens.
 */
export async function getAccessToken(): Promise<string | null> {
  if (cached && cached.expiresAt - RENEW_MARGIN_MS > Date.now()) {
    return cached.token
  }

  inFlight ??= fetchToken().finally(() => {
    inFlight = null
  })

  return inFlight
}

/** Called on sign-out, and whenever the API rejects a token as invalid. */
export function clearAccessToken() {
  cached = null
}
