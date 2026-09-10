import { getAccessToken, clearAccessToken } from './token'
import { getActiveOrg } from './activeOrg'

/** An API error carrying the envelope the backend always returns. */
export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

interface Options {
  method?: string
  body?: unknown
  /** Send no organization header. For endpoints about the person rather than
   *  a tenant: /me, creating an organization. */
  withoutOrg?: boolean
}

async function request<T>(path: string, options: Options = {}): Promise<T> {
  const token = await getAccessToken()
  const headers: Record<string, string> = {}

  if (token) headers.Authorization = `Bearer ${token}`

  if (!options.withoutOrg) {
    const org = getActiveOrg()
    if (org) headers['X-Organization-Id'] = org
  }

  if (options.body !== undefined) headers['Content-Type'] = 'application/json'

  const res = await fetch(`/api/v1${path}`, {
    method: options.method ?? 'GET',
    headers,
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  })

  if (res.status === 401) {
    // The token was rejected. Drop it so the next call mints a fresh one
    // rather than retrying the same dead credential forever.
    clearAccessToken()
  }

  if (!res.ok) {
    let code = 'UNKNOWN'
    let message = `${options.method ?? 'GET'} ${path} failed with ${res.status}`
    try {
      const body = (await res.json()) as { error?: { code?: string; message?: string } }
      if (body.error?.code) code = body.error.code
      if (body.error?.message) message = body.error.message
    } catch {
      // Not every failure has a JSON body; the status is still useful.
    }
    throw new ApiError(res.status, code, message)
  }

  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export const api = {
  get: <T>(path: string, options?: Options) => request<T>(path, { ...options, method: 'GET' }),
  post: <T>(path: string, body?: unknown, options?: Options) =>
    request<T>(path, { ...options, method: 'POST', body }),
  patch: <T>(path: string, body?: unknown, options?: Options) =>
    request<T>(path, { ...options, method: 'PATCH', body }),
  delete: <T>(path: string, options?: Options) =>
    request<T>(path, { ...options, method: 'DELETE' }),
}
