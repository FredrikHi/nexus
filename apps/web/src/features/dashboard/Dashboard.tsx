import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type { HealthStatus, Incident, IntegrationHealth, System } from '../../lib/types'
import { Card, EmptyState, HealthBadge, IncidentStatusBadge, PageHeader, SeverityBadge } from '../../components/ui'
import { formatPercent, relativeTime } from '../../lib/format'
import { ErrorRateChart, VolumeChart, WindowPicker } from '../charts/LazyCharts'
import { useSeries } from '../charts/useSeries'

// Worst first: a dashboard should lead with what needs attention.
const ORDER: HealthStatus[] = ['UNHEALTHY', 'DEGRADED', 'UNKNOWN', 'HEALTHY']

export function Dashboard() {
  const [hours, setHours] = useState(24)
  const overview = useQuery({
    queryKey: ['health-overview'],
    queryFn: () => api.get<Record<string, number>>('/integration-health/overview'),
    refetchInterval: 30_000,
  })
  const health = useQuery({
    queryKey: ['integration-health'],
    queryFn: () => api.get<IntegrationHealth[]>('/integration-health'),
    refetchInterval: 30_000,
  })
  const incidents = useQuery({
    queryKey: ['incidents', 'open'],
    queryFn: () => api.get<Incident[]>('/incidents?only_open=true'),
    refetchInterval: 30_000,
  })
  const systems = useQuery({
    queryKey: ['systems'],
    queryFn: () => api.get<System[]>('/systems'),
  })

  const nothingModelled = !systems.isPending && (systems.data?.length ?? 0) === 0

  return (
    <div>
      <PageHeader
        title="Dashboard"
        subtitle="Health is recomputed on a timer, so this shows the last evaluation rather than this instant."
      />

      {nothingModelled && (
        <div className="mb-6">
          <EmptyState
            title="Nothing is modelled yet."
            hint="Add a system, then the components inside it, then the integrations between them."
          />
        </div>
      )}

      <div className="mb-6 grid grid-cols-2 gap-3 sm:grid-cols-4">
        {ORDER.map((status) => (
          <Card key={status}>
            <div className="text-2xl font-semibold text-neutral-100">
              {overview.data?.[status] ?? 0}
            </div>
            <div className="mt-1">
              <HealthBadge status={status} />
            </div>
          </Card>
        ))}
      </div>

      <TrafficCharts hours={hours} onWindowChange={setHours} />

      <div className="grid gap-6 lg:grid-cols-2">
        <section>
          <h2 className="mb-2 text-sm font-medium text-neutral-300">Open incidents</h2>
          {(incidents.data?.length ?? 0) === 0 ? (
            <EmptyState title="No open incidents." />
          ) : (
            <div className="space-y-2">
              {incidents.data?.map((incident) => (
                <div
                  key={incident.id}
                  className="rounded-lg border border-neutral-800 bg-neutral-900/40 p-3"
                >
                  <div className="flex items-center gap-2">
                    <SeverityBadge severity={incident.severity} />
                    <IncidentStatusBadge status={incident.status} />
                    <span className="ml-auto text-xs text-neutral-500">
                      {relativeTime(incident.opened_at)}
                    </span>
                  </div>
                  <div className="mt-2 text-sm text-neutral-100">{incident.title}</div>
                  <div className="mt-0.5 text-xs text-neutral-500">{incident.summary}</div>
                </div>
              ))}
            </div>
          )}
        </section>

        <section>
          <h2 className="mb-2 text-sm font-medium text-neutral-300">Integration health</h2>
          {(health.data?.length ?? 0) === 0 ? (
            <EmptyState
              title="Nothing evaluated yet."
              hint="Health appears once an integration exists and telemetry has arrived for it."
            />
          ) : (
            <div className="divide-y divide-neutral-800 rounded-lg border border-neutral-800">
              {health.data?.map((h) => (
                <div key={h.integration_id} className="flex items-start gap-3 p-3">
                  <HealthBadge status={h.status} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm text-neutral-100">{h.integration_name}</div>
                    <div className="mt-0.5 text-xs text-neutral-400">{h.reason}</div>
                    <div className="mt-0.5 text-xs text-neutral-600">
                      {h.event_count} events, {formatPercent(h.error_rate)} errors, since{' '}
                      {relativeTime(h.since)}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  )
}

/** Traffic across every integration in the organization. */
function TrafficCharts({
  hours,
  onWindowChange,
}: {
  hours: number
  onWindowChange: (hours: number) => void
}) {
  const series = useSeries(undefined, hours)
  const points = series.data ?? []

  return (
    <div className="mb-6">
      <div className="mb-2 flex items-center justify-between">
        <h2 className="text-sm font-medium text-neutral-300">Traffic</h2>
        <WindowPicker hours={hours} onChange={onWindowChange} />
      </div>

      {points.length === 0 ? (
        <EmptyState
          title="No telemetry in this window."
          hint="Widen the window, or send events with an API key."
        />
      ) : (
        <div className="grid gap-4 lg:grid-cols-2">
          <Card>
            <div className="mb-2 text-xs text-neutral-500">Calls by outcome</div>
            <VolumeChart data={points} hours={hours} />
          </Card>
          <Card>
            <div className="mb-2 text-xs text-neutral-500">Error rate and p95 latency</div>
            <ErrorRateChart data={points} hours={hours} />
          </Card>
        </div>
      )}
    </div>
  )
}
