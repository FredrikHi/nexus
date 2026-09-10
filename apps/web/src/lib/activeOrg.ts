// Which organization the user is currently looking at.
//
// Kept in localStorage rather than the URL so a refresh lands where you were,
// and read synchronously so the first render already knows. The API verifies
// membership on every request, so a stale or tampered value here fails with a
// 403 rather than exposing anything.

const KEY = 'active-organization-id'

export function getActiveOrg(): string | null {
  try {
    return localStorage.getItem(KEY)
  } catch {
    return null
  }
}

export function setActiveOrg(id: string | null) {
  try {
    if (id) localStorage.setItem(KEY, id)
    else localStorage.removeItem(KEY)
  } catch {
    // Private browsing can refuse storage. The app still works; the choice
    // just does not survive a refresh.
  }
}
