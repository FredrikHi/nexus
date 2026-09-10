import { createAuthClient } from 'better-auth/react'

// Same origin as the app: Vite proxies /api/auth to the auth service in dev,
// and nginx does the same in the container. That keeps the session cookie
// first-party, which browsers increasingly require.
export const authClient = createAuthClient({
  basePath: '/api/auth',
})

export const { signIn, signUp, signOut, useSession } = authClient
