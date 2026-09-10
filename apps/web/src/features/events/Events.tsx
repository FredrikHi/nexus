import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type {
  Environment, Integration, TelemetryEvent, TelemetryStatus,
} from '../../lib/types'
import { EmptyState, PageHeader } from '../../components/ui'
import { TelemetryStatusBadge } from '../../components/StatusBadge'
import { formatMs, inputClass, relativeTime } from '../../lib/format'

const STATUSES: TelemetryStatus[] = ['SUCCESS', 'FAILURE', 'TIMEOUT', 'REJECTED']

export function Events() {
  const [integrationId, setIntegrationId] = useState('')
  const [environmentId, setEnvironmentId] = useState('')
  const [status, setStatus] = useState('')

  const integrations = useQuery({
    queryKey: ['integrations'],
    queryFn: () => api.get<Integration[]>('/integrations'),
  })
  const environments = useQuery({
    queryKey: ['environments'],
    queryFn: () => api.get<Environment[]>('/environments'),
  })

  const params = new URLSearchParams({ limit: '200' })
  if (integrationId) params.set('integration_id', integrationId)
  if (environmentId) params.set('environment_id', environmentId)
  if (status) params.set('status', status)

  const events = useQuery({
    queryKey: ['telemetry', integrationId, environmentId, status],
    queryFn: () => api.get<TelemetryEvent[]>(`/telemetry?${params}`),
    refetchInterval: 15_000,
  })

  const nameOf = (id: string) =>
    integrations.data?.find((i) => i.id === id)?.name ?? id.slice(0, 8)

  return (
    <div>
      <PageHeader
        title="Events"
        subtitle="Individual recorded calls, newest first. This is the raw material health and traces are derived from."
      />

      <div className="mb-4 flex flex-wrap gap-2">
        <select
          className={`${inputClass} w-56`}
          value={integrationId}
          onChange={(e) => setIntegrationId(e.target.value)}
        >
          <option value="">All integrations</option>
          {integrations.data?.map((i) => (
            <option key={i.id} value={i.id}>{i.name}</option>
          ))}
        </select>
        <select
          className={`${inputClass} w-44`}
          value={environmentId}
          onChange={(e) => setEnvironmentId(e.target.value)}
        >
          <option value="">All environments</option>
          {environments.data?.map((e) => (
            <option key={e.id} value={e.id}>{e.name}</option>
          ))}
        </select>
        <select
          className={`${inputClass} w-40`}
          value={status}
          onChange={(e) => setStatus(e.target.value)}
        >
          <option value="">Any outcome</option>
          {STATUSES.map((s) => (
            <option key={s} value={s}>{s}</option>
          ))}
        </select>
      </div>

      {events.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {events.data?.length === 0 && (
        <EmptyState
          title="No events match."
          hint="Send telemetry with an API key, or widen the filters."
        />
      )}

      {(events.data?.length ?? 0) > 0 && (
        <div className="overflow-x-auto rounded-lg border border-neutral-800">
          <table className="w-full text-left text-sm">
            <thead className="border-b border-neutral-800 text-xs uppercase tracking-wide text-neutral-500">
              <tr>
                <th className="px-3 py-2 font-medium">When</th>
                <th className="px-3 py-2 font-medium">Outcome</th>
                <th className="px-3 py-2 font-medium">Integration</th>
                <th className="px-3 py-2 font-medium">Operation</th>
                <th className="px-3 py-2 text-right font-medium">Duration</th>
                <th className="px-3 py-2 font-medium">Detail</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-neutral-800">
              {events.data?.map((e) => (
                <tr key={e.id} className="text-neutral-300">
                  <td className="whitespace-nowrap px-3 py-2 text-neutral-500">
                    {relativeTime(e.occurred_at)}
                  </td>
                  <td className="px-3 py-2"><TelemetryStatusBadge status={e.status} /></td>
                  <td className="px-3 py-2">{nameOf(e.integration_id)}</td>
                  <td className="px-3 py-2 text-neutral-400">{e.operation ?? '-'}</td>
                  <td className="whitespace-nowrap px-3 py-2 text-right text-neutral-400">
                    {formatMs(e.duration_ms)}
                  </td>
                  <td className="px-3 py-2 text-xs text-neutral-500">
                    {e.error_message
                      ? <span className="text-red-400">{e.error_type}: {e.error_message}</span>
                      : e.status_code ?? '-'}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <p className="mt-3 text-xs text-neutral-600">
        Showing at most 200 events. Older ones are dropped whole partitions at a
        time by the retention policy.
      </p>
    </div>
  )
}
