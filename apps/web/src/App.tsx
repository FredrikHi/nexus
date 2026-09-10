import { NavLink, Route, Routes } from 'react-router-dom'
import { AuthGate } from './features/auth/AuthGate'
import { OrgSwitcher } from './features/organizations/OrgSwitcher'
import { Dashboard } from './features/dashboard/Dashboard'
import { Systems } from './features/systems/Systems'
import { Integrations } from './features/integrations/Integrations'
import { Incidents } from './features/incidents/Incidents'
import { useHealth } from './features/health/useHealth'
import { signOut } from './lib/authClient'
import { clearAccessToken } from './lib/token'
import { setActiveOrg } from './lib/activeOrg'
import type { Me } from './lib/types'

const NAV: { section: string | null; items: { to: string; label: string; end?: boolean }[] }[] = [
  { section: null, items: [{ to: '/', label: 'Dashboard', end: true }] },
  { section: 'Landscape', items: [
    { to: '/systems', label: 'Systems' },
    { to: '/integrations', label: 'Integrations' },
  ] },
  { section: 'Operations', items: [{ to: '/incidents', label: 'Incidents' }] },
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

      <main className="flex-1 overflow-auto p-8">
        <Routes>
          <Route path="/" element={<Dashboard />} />
          <Route path="/systems" element={<Systems />} />
          <Route path="/integrations" element={<Integrations />} />
          <Route path="/incidents" element={<Incidents />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </main>
    </div>
  )
}

export default function App() {
  return <AuthGate>{(me) => <Shell me={me} />}</AuthGate>
}
