import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import { setActiveOrg } from '../../lib/activeOrg'
import type { Organization } from '../../lib/types'

/** Shown when someone has no organizations yet, and from the switcher. */
export function CreateOrganization({ onDone }: { onDone?: () => void }) {
  const [name, setName] = useState('')
  const queryClient = useQueryClient()

  const create = useMutation({
    mutationFn: () =>
      api.post<Organization>('/organizations', { name }, { withoutOrg: true }),
    onSuccess: (org) => {
      // Switch to what you just made: creating one and staying elsewhere
      // would be surprising.
      setActiveOrg(org.id)
      void queryClient.invalidateQueries()
      onDone?.()
    },
  })

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault()
        if (name.trim()) create.mutate()
      }}
      className="space-y-3"
    >
      <label className="block">
        <span className="mb-1 block text-xs text-neutral-400">Organization name</span>
        <input
          autoFocus
          className="w-full rounded border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm text-neutral-100 outline-none focus:border-neutral-500"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Cookly"
        />
      </label>

      {create.error && (
        <p className="text-sm text-red-400">
          {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
        </p>
      )}

      <button
        type="submit"
        disabled={create.isPending || !name.trim()}
        className="w-full rounded bg-neutral-100 px-3 py-2 text-sm font-medium text-neutral-900 hover:bg-white disabled:opacity-50"
      >
        {create.isPending ? 'Creating…' : 'Create organization'}
      </button>
    </form>
  )
}
