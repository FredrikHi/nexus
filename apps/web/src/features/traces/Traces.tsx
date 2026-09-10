import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type { Integration, TraceDetail, TraceSummary } from '../../lib/types'
import { Card, EmptyState, PageHeader } from '../../components/ui'
import { TelemetryStatusBadge } from '../../components/StatusBadge'
import { formatMs, relativeTime } from '../../lib/format'

export function Traces() {
  const [onlyErrors, setOnlyErrors] = useState(false)

  const traces = useQuery({
    queryKey: ['traces', onlyErrors],
    queryFn: () =>
      api.get<TraceSummary[]>(`/traces${onlyErrors ? '?only_errors=true' : ''}`),
    refetchInterval: 30_000,
  })

  return (
    <div>
      <PageHeader
        title="Traces"
        subtitle="One correlated flow across integrations. A trace is every event sharing a trace id, and it is only as healthy as its worst hop."
        action={
          <button
            className="rounded border border-neutral-700 px-3 py-1.5 text-sm text-neutral-300 hover:bg-neutral-900"
            onClick={() => setOnlyErrors(!onlyErrors)}
          >
            {onlyErrors ? 'Show all' : 'Failures only'}
          </button>
        }
      />

      {traces.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {traces.data?.length === 0 && (
        <EmptyState
          title={onlyErrors ? 'No failing traces.' : 'No traces recorded.'}
          hint="Traces appear when ingested events carry a trace_id."
        />
      )}

      <div className="space-y-2">
        {traces.data?.map((trace) => <TraceRow key={trace.trace_id} trace={trace} />)}
      </div>
    </div>
  )
}

function TraceRow({ trace }: { trace: TraceSummary }) {
  const [open, setOpen] = useState(false)

  return (
    <Card>
      <button className="flex w-full items-center gap-3 text-left" onClick={() => setOpen(!open)}>
        <span className="text-neutral-600">{open ? '▾' : '▸'}</span>
        <TelemetryStatusBadge status={trace.status} />
        <span className="font-mono text-xs text-neutral-300">{trace.trace_id}</span>
        <span className="ml-auto flex items-center gap-4 text-xs text-neutral-500">
          <span>{trace.span_count} spans</span>
          <span>{trace.integration_count} integrations</span>
          {trace.error_count > 0 && (
            <span className="text-red-400">{trace.error_count} failed</span>
          )}
          <span>{formatMs(trace.elapsed_ms)}</span>
          <span>{relativeTime(trace.started_at)}</span>
        </span>
      </button>

      {open && <Waterfall traceId={trace.trace_id} />}
    </Card>
  )
}

/**
 * The spans of a trace, drawn as a waterfall.
 *
 * Each bar is positioned by when the span started relative to the trace, and
 * sized by how long it took. That is the view that makes a slow hop obvious,
 * which a plain list never does.
 */
function Waterfall({ traceId }: { traceId: string }) {
  const detail = useQuery({
    queryKey: ['trace', traceId],
    queryFn: () => api.get<TraceDetail>(`/traces/${encodeURIComponent(traceId)}`),
  })
  const integrations = useQuery({
    queryKey: ['integrations'],
    queryFn: () => api.get<Integration[]>('/integrations'),
  })

  if (detail.isPending) return <p className="mt-3 text-sm text-neutral-600">Loading spans…</p>
  if (!detail.data) return null

  const spans = detail.data.spans
  const start = new Date(detail.data.started_at).getTime()
  // Guard against a zero-width trace: every span at the same millisecond.
  const span = Math.max(1, new Date(detail.data.ended_at).getTime() - start)

  const nameOf = (id: string) =>
    integrations.data?.find((i) => i.id === id)?.name ?? id.slice(0, 8)

  return (
    <div className="mt-4 space-y-1.5 border-t border-neutral-800 pt-4">
      {spans.map((s) => {
        const offset = ((new Date(s.occurred_at).getTime() - start) / span) * 100
        const width = Math.max(1.5, ((s.duration_ms ?? 0) / span) * 100)
        const color =
          s.status === 'SUCCESS' ? 'bg-emerald-500'
          : s.status === 'FAILURE' ? 'bg-red-500'
          : s.status === 'TIMEOUT' ? 'bg-orange-500'
          : 'bg-neutral-500'

        return (
          <div key={s.id} className="flex items-center gap-3">
            <div className="w-52 shrink-0 truncate text-xs text-neutral-400">
              {nameOf(s.integration_id)}
            </div>
            <div className="relative h-4 flex-1 rounded bg-neutral-900">
              <div
                className={`absolute h-4 rounded ${color}`}
                style={{ left: `${Math.min(offset, 98)}%`, width: `${Math.min(width, 100 - offset)}%` }}
                title={`${s.status} ${formatMs(s.duration_ms)}`}
              />
            </div>
            <div className="w-16 shrink-0 text-right text-xs text-neutral-500">
              {formatMs(s.duration_ms)}
            </div>
          </div>
        )
      })}

      {spans.some((s) => s.error_message) && (
        <div className="mt-3 space-y-1">
          {spans
            .filter((s) => s.error_message)
            .map((s) => (
              <p key={s.id} className="text-xs text-red-400">
                {nameOf(s.integration_id)}: {s.error_type} {s.error_message}
              </p>
            ))}
        </div>
      )}
    </div>
  )
}
