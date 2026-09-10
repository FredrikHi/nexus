import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type {
  Component, Integration, IntegrationHealth, System, TelemetrySummary,
} from '../../lib/types'
import { Badge, Card, EmptyState, HealthBadge, PageHeader } from '../../components/ui'
import { buttonClass, formatMs, formatPercent, relativeTime } from '../../lib/format'
import { CRITICALITY_STYLES, humanise } from '../systems/constants'
import { NewIntegrationForm } from './NewIntegrationForm'

export function Integrations() {
  const [adding, setAdding] = useState(false)

  const integrations = useQuery({
    queryKey: ['integrations'],
    queryFn: () => api.get<Integration[]>('/integrations'),
  })
  const health = useQuery({
    queryKey: ['integration-health'],
    queryFn: () => api.get<IntegrationHealth[]>('/integration-health'),
    refetchInterval: 30_000,
  })
  const components = useQuery({
    queryKey: ['components'],
    queryFn: () => api.get<Component[]>('/components'),
  })
  const systems = useQuery({
    queryKey: ['systems'],
    queryFn: () => api.get<System[]>('/systems'),
  })

  // Health is a separate resource, so join it here by integration id rather
  // than asking the API for a combined shape it does not offer.
  const healthFor = (id: string) => health.data?.find((h) => h.integration_id === id)

  const label = (componentId: string) => {
    const component = components.data?.find((c) => c.id === componentId)
    if (!component) return 'unknown'
    const system = systems.data?.find((s) => s.id === component.system_id)
    return `${system?.name ?? '?'} / ${component.name}`
  }

  return (
    <div>
      <PageHeader
        title="Integrations"
        subtitle="A directed edge between two components. This is what health and incidents attach to."
        action={
          <button className={buttonClass} onClick={() => setAdding(!adding)}>
            {adding ? 'Cancel' : 'Add integration'}
          </button>
        }
      />

      {adding && (
        <Card className="mb-6">
          <NewIntegrationForm onDone={() => setAdding(false)} />
        </Card>
      )}

      {integrations.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {integrations.data?.length === 0 && !adding && (
        <EmptyState
          title="No integrations yet."
          hint="Connect two components to start collecting telemetry between them."
        />
      )}

      <div className="space-y-3">
        {integrations.data?.map((integration) => (
          <IntegrationRow
            key={integration.id}
            integration={integration}
            health={healthFor(integration.id)}
            from={label(integration.source_component_id)}
            to={label(integration.destination_component_id)}
          />
        ))}
      </div>
    </div>
  )
}

function IntegrationRow({
  integration,
  health,
  from,
  to,
}: {
  integration: Integration
  health: IntegrationHealth | undefined
  from: string
  to: string
}) {
  const [open, setOpen] = useState(false)

  return (
    <Card>
      <button className="flex w-full items-start gap-3 text-left" onClick={() => setOpen(!open)}>
        <span className="mt-1 text-neutral-600">{open ? '▾' : '▸'}</span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium text-neutral-100">{integration.name}</span>
            {health ? (
              <HealthBadge status={health.status} />
            ) : (
              <Badge className="border-neutral-700 bg-neutral-800 text-neutral-500">
                not evaluated
              </Badge>
            )}
            <Badge className={CRITICALITY_STYLES[integration.criticality]}>
              {humanise(integration.criticality)}
            </Badge>
            {!integration.monitoring_enabled && (
              <Badge className="border-neutral-700 bg-neutral-800 text-neutral-500">
                monitoring off
              </Badge>
            )}
          </div>
          <p className="mt-1 text-sm text-neutral-500">
            {from} <span className="text-neutral-600">to</span> {to}
          </p>
          {health && <p className="mt-1 text-xs text-neutral-600">{health.reason}</p>}
        </div>
      </button>

      {open && (
        <div className="mt-4 border-t border-neutral-800 pt-4 pl-6">
          <TelemetryPanel integrationId={integration.id} health={health} />
        </div>
      )}
    </Card>
  )
}

function TelemetryPanel({
  integrationId,
  health,
}: {
  integrationId: string
  health: IntegrationHealth | undefined
}) {
  const summary = useQuery({
    queryKey: ['telemetry-summary', integrationId],
    queryFn: () =>
      api.get<TelemetrySummary>(`/telemetry/summary?integration_id=${integrationId}`),
  })

  if (summary.isPending) return <p className="text-sm text-neutral-600">Loading…</p>

  const s = summary.data
  if (!s || s.total === 0) {
    return (
      <p className="text-sm text-neutral-600">
        No telemetry recorded. Send events with an API key to see health here.
      </p>
    )
  }

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      <Stat label="Events" value={String(s.total)} />
      <Stat label="Error rate" value={formatPercent(s.error_rate)} />
      <Stat label="p95" value={formatMs(s.p95_duration_ms)} />
      <Stat label="Last event" value={relativeTime(health?.last_event_at ?? null)} />
      <Stat label="Success" value={String(s.success)} />
      <Stat label="Failure" value={String(s.failure)} />
      <Stat label="Timeout" value={String(s.timeout)} />
      <Stat label="Rejected" value={String(s.rejected)} />
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div className="text-xs text-neutral-500">{label}</div>
      <div className="text-sm text-neutral-100">{value}</div>
    </div>
  )
}
