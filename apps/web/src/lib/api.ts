// Tiny typed fetch wrapper. All calls are relative to /api/v1, which the
// Vite dev proxy (and nginx in the container) forwards to the Rust API.
export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetch(`/api/v1${path}`)
  if (!res.ok) {
    throw new Error(`GET ${path} failed with ${res.status}`)
  }
  return (await res.json()) as T
}
