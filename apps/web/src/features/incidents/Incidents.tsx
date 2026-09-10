import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { Incident } from '../../lib/types'
import { Card, EmptyState, IncidentStatusBadge, PageHeader, SeverityBadge } from '../../components/ui'
import { buttonClass, relativeTime } from '../../lib/format'

export function Incidents() {
  const [onlyOpen, setOnlyOpen] = useState(true)

  const incidents = useQuery({
    queryKey: ['incidents', onlyOpen],
    queryFn: () => api.get<Incident[]>(`/incidents${onlyOpen ? '?only_open=true' : ''}`),
    refetchInterval: 30_000,
  })

  return (
    <div>
      <PageHeader
        title="Incidents"
        subtitle="Opened and resolved automatically from health. You acknowledge and annotate; recovery is what closes one."
        action={
          <button
            className="rounded border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-900"
            onClick={() => setOnlyOpen(!onlyOpen)}
          >
            {onlyOpen ? 'Show all' : 'Show open only'}
          </button>
        }
      />

      {incidents.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {incidents.data?.length === 0 && (
        <EmptyState title={onlyOpen ? 'Nothing is broken right now.' : 'No incidents recorded.'} />
      )}

      <div className="space-y-3">
        {incidents.data?.map((incident) => (
          <IncidentRow key={incident.id} incident={incident} />
        ))}
      </div>
    </div>
  )
}

function IncidentRow({ incident }: { incident: Incident }) {
  const queryClient = useQueryClient()

  const acknowledge = useMutation({
    mutationFn: () => api.post(`/incidents/${incident.id}/acknowledge`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['incidents'] }),
  })

  return (
    <Card>
      <div className="flex flex-wrap items-center gap-2">
        <SeverityBadge severity={incident.severity} />
        <IncidentStatusBadge status={incident.status} />
        <span className="text-sm font-medium text-neutral-100">{incident.title}</span>
        <span className="ml-auto text-xs text-neutral-500">
          opened {relativeTime(incident.opened_at)}
          {incident.resolved_at ? `, resolved ${relativeTime(incident.resolved_at)}` : ''}
        </span>
      </div>

      <p className="mt-2 text-sm text-neutral-400">{incident.summary}</p>

      {incident.acknowledged_by && (
        <p className="mt-1 text-xs text-neutral-600">
          Acknowledged by {incident.acknowledged_by} {relativeTime(incident.acknowledged_at)}
        </p>
      )}

      {incident.status === 'OPEN' && (
        <button
          className={`${buttonClass} mt-3`}
          disabled={acknowledge.isPending}
          onClick={() => acknowledge.mutate()}
        >
          {acknowledge.isPending ? 'Acknowledging…' : 'Acknowledge'}
        </button>
      )}

      {acknowledge.error && (
        <p className="mt-2 text-sm text-red-400">
          {acknowledge.error instanceof ApiError
            ? acknowledge.error.message
            : 'Could not acknowledge it.'}
        </p>
      )}
    </Card>
  )
}
