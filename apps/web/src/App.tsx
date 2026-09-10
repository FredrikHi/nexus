import { Suspense, lazy } from 'react'
import { NavLink, Route, Routes } from 'react-router-dom'
import { AuthGate } from './features/auth/AuthGate'
import { OrgSwitcher } from './features/organizations/OrgSwitcher'
import { Dashboard } from './features/dashboard/Dashboard'
import { Systems } from './features/systems/Systems'
import { Integrations } from './features/integrations/Integrations'
import { Incidents } from './features/incidents/Incidents'

// Split by route. The flow graph pulls in a whole graph library and the charts
// pull in a charting one; neither belongs in the bundle someone downloads just
// to reach the sign-in screen.
const Flow = lazy(() => import('./features/flow/Flow').then((m) => ({ default: m.Flow })))
const Traces = lazy(() => import('./features/traces/Traces').then((m) => ({ default: m.Traces })))
const Events = lazy(() => import('./features/events/Events').then((m) => ({ default: m.Events })))
const Teams = lazy(() => import('./features/admin/Teams').then((m) => ({ default: m.Teams })))
const Environments = lazy(() =>
  import('./features/admin/Environments').then((m) => ({ default: m.Environments })),
)
const IntegrationTypes = lazy(() =>
  import('./features/admin/IntegrationTypes').then((m) => ({ default: m.IntegrationTypes })),
)
import { useHealth } from './features/health/useHealth'
import { signOut } from './lib/authClient'
import { clearAccessToken } from './lib/token'
import { setActiveOrg } from './lib/activeOrg'
import type { Me } from './lib/types'

const NAV: { section: string | null; items: { to: string; label: string; end?: boolean }[] }[] = [
  { section: null, items: [
    { to: '/', label: 'Dashboard', end: true },
    { to: '/flow', label: 'Flow' },
  ] },
  { section: 'Landscape', items: [
    { to: '/systems', label: 'Systems' },
    { to: '/integrations', label: 'Integrations' },
  ] },
  { section: 'Observability', items: [
    { to: '/traces', label: 'Traces' },
    { to: '/events', label: 'Events' },
  ] },
  { section: 'Operations', items: [{ to: '/incidents', label: 'Incidents' }] },
  { section: 'Administration', items: [
    { to: '/teams', label: 'Teams' },
    { to: '/environments', label: 'Environments' },
    { to: '/integration-types', label: 'Integration Types' },
  ] },
]

function ApiPill() {
  const { data, isLoading, isError } = useHealth()
  const ok = data?.status === 'ok'
  const color = isLoading ? 'bg-neutral-500' : ok ? 'bg-emerald-500' : 'bg-red-500'
  const label = isLoading
    ? 'checking'
    : isError ? 'API unreachable' : ok ? 'API healthy' : 'API degraded'
  return (
    <div className="flex items-center gap-2 text-xs text-neutral-400">
      <span className={`inline-block h-2 w-2 rounded-full ${color}`} />
      {label}
    </div>
  )
}

function NotFound() {
  return (
    <div>
      <h1 className="text-xl font-semibold text-neutral-100">Not found</h1>
      <p className="mt-2 text-sm text-neutral-400">No such page.</p>
    </div>
  )
}

function Shell({ me }: { me: Me }) {
  async function handleSignOut() {
    // Drop the cached bearer token and the remembered tenant before the
    // session goes, so nothing keeps using them mid-teardown.
    clearAccessToken()
    setActiveOrg(null)
    await signOut()
  }

  return (
    <div className="flex h-full bg-neutral-950 text-neutral-200">
      <aside className="flex w-60 shrink-0 flex-col border-r border-neutral-800 p-4">
        <div className="mb-4 text-sm font-semibold tracking-wide text-neutral-100">
          Integration Control Center
        </div>

        <div className="mb-6">
          <OrgSwitcher organizations={me.organizations} />
        </div>

        <nav className="flex-1 space-y-4">
          {NAV.map((group, i) => (
            <div key={i}>
              {group.section && (
                <div className="mb-1 px-2 text-[10px] font-medium uppercase tracking-wider text-neutral-600">
                  {group.section}
                </div>
              )}
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) =>
                    `block rounded px-2 py-1.5 text-sm ${
                      isActive
                        ? 'bg-neutral-800 text-neutral-100'
                        : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
                    }`
                  }
                >
                  {item.label}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>

        <div className="mt-4 border-t border-neutral-800 pt-4">
          <ApiPill />
          <div className="mt-3 truncate text-xs text-neutral-500">{me.email}</div>
          <button
            onClick={handleSignOut}
            className="mt-1 text-xs text-neutral-400 hover:text-neutral-200"
          >
            Sign out
          </button>
        </div>
      </aside>

      <main className="flex flex-1 flex-col overflow-auto p-8">
        <Suspense fallback={<p className="text-sm text-neutral-500">Loading…</p>}>
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/systems" element={<Systems />} />
          <Route path="/integrations" element={<Integrations />} />
          <Route path="/flow" element={<Flow />} />
          <Route path="/traces" element={<Traces />} />
          <Route path="/events" element={<Events />} />
          <Route path="/incidents" element={<Incidents />} />
          <Route path="/teams" element={<Teams />} />
          <Route path="/environments" element={<Environments />} />
          <Route path="/integration-types" element={<IntegrationTypes />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
        </Suspense>
      </main>
    </div>
  )
}

export default function App() {
  return <AuthGate>{(me) => <Shell me={me} />}</AuthGate>
}
