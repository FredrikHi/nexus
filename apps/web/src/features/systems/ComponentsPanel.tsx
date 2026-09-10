import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { Component, ComponentType } from '../../lib/types'
import { Badge } from '../../components/ui'
import { buttonClass, inputClass } from '../../lib/format'
import { COMPONENT_TYPES, humanise } from './constants'

/** The components inside one system, loaded only when its row is expanded. */
export function ComponentsPanel({ systemId }: { systemId: string }) {
  const [name, setName] = useState('')
  const [type, setType] = useState<ComponentType>('SERVICE')
  const queryClient = useQueryClient()

  const components = useQuery({
    queryKey: ['components', systemId],
    queryFn: () => api.get<Component[]>(`/components?system_id=${systemId}`),
  })

  const create = useMutation({
    mutationFn: () =>
      api.post<Component>('/components', { system_id: systemId, name, component_type: type }),
    onSuccess: () => {
      setName('')
      void queryClient.invalidateQueries({ queryKey: ['components', systemId] })
    },
  })

  return (
    <div>
      <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-neutral-500">
        Components
      </h3>

      {components.isPending && <p className="text-sm text-neutral-600">Loading…</p>}
      {components.data?.length === 0 && (
        <p className="mb-3 text-sm text-neutral-600">
          None yet. Integrations connect components rather than systems, so add at least one.
        </p>
      )}

      <div className="mb-3 space-y-1">
        {components.data?.map((c) => (
          <div key={c.id} className="flex items-center gap-2 text-sm text-neutral-300">
            <span>{c.name}</span>
            <Badge className="border-neutral-700 bg-neutral-800 text-neutral-500">
              {humanise(c.component_type)}
            </Badge>
          </div>
        ))}
      </div>

      <form
        className="flex flex-wrap gap-2"
        onSubmit={(e) => {
          e.preventDefault()
          if (name.trim()) create.mutate()
        }}
      >
        <input
          className={`${inputClass} flex-1 min-w-40`}
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="New component"
        />
        <select
          className={`${inputClass} w-40`}
          value={type}
          onChange={(e) => setType(e.target.value as ComponentType)}
        >
          {COMPONENT_TYPES.map((t) => (
            <option key={t} value={t}>{humanise(t)}</option>
          ))}
        </select>
        <button className={buttonClass} disabled={create.isPending || !name.trim()}>
          Add
        </button>
      </form>

      {create.error && (
        <p className="mt-2 text-sm text-red-400">
          {create.error instanceof ApiError ? create.error.message : 'Could not add it.'}
        </p>
      )}
    </div>
  )
}
