import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type {
  Component, Criticality, Environment, Integration, IntegrationType, System,
} from '../../lib/types'
import { buttonClass, inputClass } from '../../lib/format'
import { CRITICALITIES, humanise } from '../systems/constants'

export function NewIntegrationForm({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState('')
  const [source, setSource] = useState('')
  const [destination, setDestination] = useState('')
  const [typeId, setTypeId] = useState('')
  const [environmentId, setEnvironmentId] = useState('')
  const [criticality, setCriticality] = useState<Criticality>('MEDIUM')
  const queryClient = useQueryClient()

  const components = useQuery({
    queryKey: ['components'],
    queryFn: () => api.get<Component[]>('/components'),
  })
  const systems = useQuery({
    queryKey: ['systems'],
    queryFn: () => api.get<System[]>('/systems'),
  })
  const types = useQuery({
    queryKey: ['integration-types'],
    queryFn: () => api.get<IntegrationType[]>('/integration-types'),
  })
  const environments = useQuery({
    queryKey: ['environments'],
    queryFn: () => api.get<Environment[]>('/environments'),
  })

  const systemName = (id: string) => systems.data?.find((s) => s.id === id)?.name ?? '?'
  const options = components.data ?? []

  const create = useMutation({
    mutationFn: () =>
      api.post<Integration>('/integrations', {
        name,
        source_component_id: source,
        destination_component_id: destination,
        integration_type_id: typeId || types.data?.[0]?.id,
        environment_id: environmentId || null,
        criticality,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['integrations'] })
      onDone()
    },
  })

  if (options.length < 2) {
    return (
      <p className="text-sm text-neutral-400">
        An integration connects two components, so you need at least two before you can create one.
        Add them under Systems.
      </p>
    )
  }

  const ready = name.trim() && source && destination && source !== destination

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault()
        if (ready) create.mutate()
      }}
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="block sm:col-span-2">
          <span className="mb-1 block text-xs text-neutral-400">Name</span>
          <input autoFocus className={inputClass} value={name} onChange={(e) => setName(e.target.value)} />
        </label>

        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">From (source)</span>
          <select className={inputClass} value={source} onChange={(e) => setSource(e.target.value)}>
            <option value="">Choose…</option>
            {options.map((c) => (
              <option key={c.id} value={c.id}>{systemName(c.system_id)} / {c.name}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">To (destination)</span>
          <select
            className={inputClass}
            value={destination}
            onChange={(e) => setDestination(e.target.value)}
          >
            <option value="">Choose…</option>
            {options.map((c) => (
              <option key={c.id} value={c.id}>{systemName(c.system_id)} / {c.name}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">Type</span>
          <select className={inputClass} value={typeId} onChange={(e) => setTypeId(e.target.value)}>
            {types.data?.map((t) => (
              <option key={t.id} value={t.id}>{t.name}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">Environment</span>
          <select
            className={inputClass}
            value={environmentId}
            onChange={(e) => setEnvironmentId(e.target.value)}
          >
            <option value="">Any</option>
            {environments.data?.map((e) => (
              <option key={e.id} value={e.id}>{e.name}</option>
            ))}
          </select>
        </label>

        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">Criticality</span>
          <select
            className={inputClass}
            value={criticality}
            onChange={(e) => setCriticality(e.target.value as Criticality)}
          >
            {CRITICALITIES.map((c) => (
              <option key={c} value={c}>{humanise(c)}</option>
            ))}
          </select>
        </label>
      </div>

      {source && source === destination && (
        <p className="text-sm text-amber-400">
          Source and destination must differ. An edge from a component to itself carries no information.
        </p>
      )}

      {create.error && (
        <p className="text-sm text-red-400">
          {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
        </p>
      )}

      <button className={buttonClass} disabled={create.isPending || !ready}>
        {create.isPending ? 'Creating…' : 'Create integration'}
      </button>
    </form>
  )
}
