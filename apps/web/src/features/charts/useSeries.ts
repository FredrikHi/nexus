import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type { SeriesPoint } from '../../lib/types'

/** Windows an operator actually asks for, in hours. */
export const WINDOWS = [
  { label: '1h', hours: 1 },
  { label: '6h', hours: 6 },
  { label: '24h', hours: 24 },
  { label: '7d', hours: 24 * 7 },
]

/**
 * Telemetry bucketed over a rolling window.
 *
 * The window start is computed inside the query function, not during render.
 * Reading the clock while rendering would produce a slightly different value
 * every time React re-runs the component, which makes the fetch unstable for
 * no benefit: the data does not change between two renders a millisecond
 * apart.
 */
export function useSeries(integrationId: string | undefined, hours: number) {
  return useQuery({
    queryKey: ['telemetry-series', integrationId ?? 'all', hours],
    queryFn: () => {
      const params = new URLSearchParams({
        from: new Date(Date.now() - hours * 3600_000).toISOString(),
      })
      if (integrationId) params.set('integration_id', integrationId)
      return api.get<SeriesPoint[]>(`/telemetry/series?${params}`)
    },
    refetchInterval: 60_000,
  })
}
