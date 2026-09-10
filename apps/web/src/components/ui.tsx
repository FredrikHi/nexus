import type { HealthStatus, IncidentStatus, Severity } from '../lib/types'

export function PageHeader({
  title,
  subtitle,
  action,
}: {
  title: string
  subtitle?: string
  action?: React.ReactNode
}) {
  return (
    <div className="mb-6 flex items-start justify-between gap-4">
      <div>
        <h1 className="text-xl font-semibold text-neutral-100">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-neutral-400">{subtitle}</p>}
      </div>
      {action}
    </div>
  )
}

export function Card({
  children,
  className = '',
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <div className={`rounded-lg border border-neutral-800 bg-neutral-900/40 p-4 ${className}`}>
      {children}
    </div>
  )
}

export function EmptyState({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="rounded-lg border border-dashed border-neutral-800 p-8 text-center">
      <p className="text-sm text-neutral-300">{title}</p>
      {hint && <p className="mt-1 text-sm text-neutral-500">{hint}</p>}
    </div>
  )
}

export function Badge({
  children,
  className = '',
}: {
  children: React.ReactNode
  className?: string
}) {
  return (
    <span
      className={`inline-block rounded border px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide ${className}`}
    >
      {children}
    </span>
  )
}

// Colours are defined once so a colour always means the same thing wherever a
// status appears.
const HEALTH_STYLES: Record<HealthStatus, string> = {
  HEALTHY: 'bg-emerald-950 text-emerald-300 border-emerald-900',
  DEGRADED: 'bg-amber-950 text-amber-300 border-amber-900',
  UNHEALTHY: 'bg-red-950 text-red-300 border-red-900',
  UNKNOWN: 'bg-neutral-800 text-neutral-400 border-neutral-700',
}

export function HealthBadge({ status }: { status: HealthStatus }) {
  return <Badge className={HEALTH_STYLES[status]}>{status}</Badge>
}

const SEVERITY_STYLES: Record<Severity, string> = {
  MINOR: 'bg-neutral-800 text-neutral-300 border-neutral-700',
  MAJOR: 'bg-amber-950 text-amber-300 border-amber-900',
  CRITICAL: 'bg-red-950 text-red-300 border-red-900',
}

export function SeverityBadge({ severity }: { severity: Severity }) {
  return <Badge className={SEVERITY_STYLES[severity]}>{severity}</Badge>
}

const INCIDENT_STYLES: Record<IncidentStatus, string> = {
  OPEN: 'bg-red-950 text-red-300 border-red-900',
  ACKNOWLEDGED: 'bg-blue-950 text-blue-300 border-blue-900',
  RESOLVED: 'bg-emerald-950 text-emerald-300 border-emerald-900',
}

export function IncidentStatusBadge({ status }: { status: IncidentStatus }) {
  return <Badge className={INCIDENT_STYLES[status]}>{status}</Badge>
}
