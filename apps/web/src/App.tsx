import { NavLink, Route, Routes } from 'react-router-dom'
import { useHealth } from './features/health/useHealth'

const NAV: { section: string | null; items: { to: string; label: string; end?: boolean }[] }[] = [
  { section: null, items: [
    { to: '/', label: 'Dashboard', end: true },
    { to: '/architecture', label: 'Architecture' },
  ] },
  { section: 'Systems', items: [
    { to: '/systems', label: 'All Systems' },
    { to: '/integrations', label: 'Integrations' },
  ] },
  { section: 'Observability', items: [
    { to: '/traces', label: 'Traces' },
    { to: '/events', label: 'Events' },
    { to: '/errors', label: 'Errors' },
  ] },
  { section: 'Operations', items: [
    { to: '/incidents', label: 'Incidents' },
    { to: '/health', label: 'Health' },
  ] },
  { section: 'Administration', items: [
    { to: '/teams', label: 'Teams' },
    { to: '/environments', label: 'Environments' },
    { to: '/integration-types', label: 'Integration Types' },
  ] },
]

function HealthPill() {
  const { data, isLoading, isError } = useHealth()
  const ok = data?.status === 'ok'
  const color = isLoading ? 'bg-neutral-500' : ok ? 'bg-emerald-500' : 'bg-red-500'
  const label = isLoading ? 'checking…' : isError ? 'API unreachable' : ok ? 'API healthy' : 'API degraded'
  return (
    <div className="flex items-center gap-2 text-sm text-neutral-300">
      <span className={`inline-block h-2 w-2 rounded-full ${color}`} />
      {label}
    </div>
  )
}

function Placeholder({ title }: { title: string }) {
  return (
    <div>
      <h1 className="text-xl font-semibold text-neutral-100">{title}</h1>
      <p className="mt-2 text-sm text-neutral-400">Nothing here yet — coming in a later phase.</p>
    </div>
  )
}

export default function App() {
  return (
    <div className="flex h-full bg-neutral-950 text-neutral-200">
      <aside className="w-60 shrink-0 border-r border-neutral-800 p-4">
        <div className="mb-6 text-sm font-semibold tracking-wide text-neutral-100">
          Integration Control Center
        </div>
        <nav className="space-y-4">
          {NAV.map((group, i) => (
            <div key={i}>
              {group.section && (
                <div className="mb-1 px-2 text-xs uppercase tracking-wider text-neutral-500">
                  {group.section}
                </div>
              )}
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) =>
                    `block rounded px-2 py-1 text-sm ${
                      isActive ? 'bg-neutral-800 text-neutral-100' : 'text-neutral-400 hover:text-neutral-200'
                    }`
                  }
                >
                  {item.label}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-neutral-800 px-6 py-3">
          <div className="text-sm text-neutral-400">Development</div>
          <HealthPill />
        </header>
        <main className="flex-1 overflow-auto p-6">
          <Routes>
            <Route path="/" element={<Placeholder title="Dashboard" />} />
            <Route path="/architecture" element={<Placeholder title="Architecture" />} />
            <Route path="/systems" element={<Placeholder title="Systems" />} />
            <Route path="/integrations" element={<Placeholder title="Integrations" />} />
            <Route path="/traces" element={<Placeholder title="Traces" />} />
            <Route path="/events" element={<Placeholder title="Events" />} />
            <Route path="/errors" element={<Placeholder title="Errors" />} />
            <Route path="/incidents" element={<Placeholder title="Incidents" />} />
            <Route path="/health" element={<Placeholder title="Health" />} />
            <Route path="/teams" element={<Placeholder title="Teams" />} />
            <Route path="/environments" element={<Placeholder title="Environments" />} />
            <Route path="/integration-types" element={<Placeholder title="Integration Types" />} />
            <Route path="*" element={<Placeholder title="Not found" />} />
          </Routes>
        </main>
      </div>
    </div>
  )
}
