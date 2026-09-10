import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { getActiveOrg, setActiveOrg } from '../../lib/activeOrg'
import { CreateOrganization } from './CreateOrganization'
import type { Membership } from '../../lib/types'

export function OrgSwitcher({ organizations }: { organizations: Membership[] }) {
  const [open, setOpen] = useState(false)
  const [creating, setCreating] = useState(false)
  const queryClient = useQueryClient()
  const activeId = getActiveOrg()
  const active = organizations.find((o) => o.organization_id === activeId)

  function choose(id: string) {
    setActiveOrg(id)
    setOpen(false)
    // Every cached query was fetched for the previous tenant, so none of it is
    // valid any more. Clearing beats invalidating: it also drops data the user
    // should no longer see while a refetch is in flight.
    queryClient.clear()
  }

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between rounded border border-neutral-800 px-2 py-1.5 text-left text-sm text-neutral-200 hover:bg-neutral-900"
      >
        <span className="truncate">{active?.organization_name ?? 'Select organization'}</span>
        <span className="ml-2 text-neutral-500">▾</span>
      </button>

      {open && (
        <div className="absolute z-20 mt-1 w-full rounded border border-neutral-800 bg-neutral-900 p-1 shadow-lg">
          {creating ? (
            <div className="p-2">
              <CreateOrganization
                onDone={() => {
                  setCreating(false)
                  setOpen(false)
                }}
              />
              <button
                onClick={() => setCreating(false)}
                className="mt-2 w-full text-xs text-neutral-500 hover:text-neutral-300"
              >
                Cancel
              </button>
            </div>
          ) : (
            <>
              {organizations.map((o) => (
                <button
                  key={o.organization_id}
                  onClick={() => choose(o.organization_id)}
                  className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-sm hover:bg-neutral-800 ${
                    o.organization_id === activeId ? 'text-neutral-100' : 'text-neutral-400'
                  }`}
                >
                  <span className="truncate">{o.organization_name}</span>
                  <span className="ml-2 shrink-0 text-[10px] uppercase tracking-wide text-neutral-600">
                    {o.role}
                  </span>
                </button>
              ))}
              <div className="my-1 h-px bg-neutral-800" />
              <button
                onClick={() => setCreating(true)}
                className="w-full rounded px-2 py-1.5 text-left text-sm text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
              >
                New organization…
              </button>
            </>
          )}
        </div>
      )}
    </div>
  )
}
