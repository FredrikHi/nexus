import { useEffect } from 'react'
import { useSession } from '../../lib/authClient'
import { useMe } from '../organizations/useMe'
import { getActiveOrg, setActiveOrg } from '../../lib/activeOrg'
import { CreateOrganization } from '../organizations/CreateOrganization'
import { SignIn } from './SignIn'
import type { Me } from '../../lib/types'

/**
 * Decides what the app is allowed to show.
 *
 * Three gates in order: signed in at all, then belongs to an organization,
 * then has one selected. Each has a different remedy, so each gets its own
 * screen rather than one generic error.
 */
export function AuthGate({ children }: { children: (me: Me) => React.ReactNode }) {
  const session = useSession()
  // Keyed on the signed-in user, so a sign-out and a different sign-in do not
  // share a cache entry.
  const me = useMe(session.data?.user?.id)

  const organizations = me.data?.organizations ?? []
  const activeId = getActiveOrg()
  const activeIsValid = organizations.some((o) => o.organization_id === activeId)

  // Pick a default once memberships are known: on first sign-in, and after
  // losing access to whatever was selected before.
  // The dependency is the id rather than the array, which is a new object on
  // every render and would re-run this effect constantly.
  const firstOrgId = organizations[0]?.organization_id
  useEffect(() => {
    if (firstOrgId && !activeIsValid) {
      setActiveOrg(firstOrgId)
    }
  }, [firstOrgId, activeIsValid])

  if (session.isPending) return <Splash>Loading…</Splash>
  if (!session.data) return <SignIn />
  // `isPending` stays true while the query is disabled, so check the data.
  if (!me.data && !me.error) return <Splash>Loading your organizations…</Splash>

  if (me.error) {
    return (
      <Splash>
        <p className="text-red-400">Could not reach the API.</p>
        <p className="mt-1 text-sm text-neutral-500">
          Check that it is running, then reload.
        </p>
      </Splash>
    )
  }

  if (!me.data) return <Splash>Loading…</Splash>

  if (organizations.length === 0) {
    return (
      <Splash>
        <div className="w-full max-w-sm text-left">
          <h1 className="text-lg font-semibold text-neutral-100">Create your first organization</h1>
          <p className="mb-4 mt-1 text-sm text-neutral-400">
            Everything you model lives inside one. You can create more later and switch between them.
          </p>
          <CreateOrganization />
        </div>
      </Splash>
    )
  }

  // The effect above sets this; render nothing rather than a flash of the app
  // with no tenant selected.
  if (!activeIsValid) return <Splash>Selecting organization…</Splash>

  return <>{children(me.data)}</>
}

function Splash({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex min-h-full items-center justify-center bg-neutral-950 p-6 text-center text-neutral-300">
      <div>{children}</div>
    </div>
  )
}
