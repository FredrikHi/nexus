import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type { Me } from '../../lib/types'

/**
 * Who the caller is and which organizations they belong to.
 *
 * Sent without an organization header on purpose: this is the call that tells
 * the client which organizations there are to choose from.
 *
 * `userId` is part of the key and gates the query. Without that, this runs
 * before anyone has signed in, fails with a 401, and stays failed: the retry
 * policy deliberately does not retry authorisation errors, so the poisoned
 * result would survive the sign-in that fixes it.
 */
export function useMe(userId: string | undefined) {
  return useQuery({
    queryKey: ['me', userId],
    queryFn: () => api.get<Me>('/me', { withoutOrg: true }),
    enabled: Boolean(userId),
    staleTime: 60_000,
  })
}
