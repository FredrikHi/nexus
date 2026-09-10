import { useQuery } from '@tanstack/react-query'
import { apiGet } from '../../lib/api'

interface HealthResponse {
  status: string
}

// TanStack Query owns this server state: caching, refetch, loading/error flags.
// We poll every 10s so the header pill reflects live API status.
export function useHealth() {
  return useQuery({
    queryKey: ['health'],
    queryFn: () => apiGet<HealthResponse>('/health'),
    refetchInterval: 10_000,
  })
}
