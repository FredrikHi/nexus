import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { Criticality, System, SystemType } from '../../lib/types'
import { buttonClass, inputClass } from '../../lib/format'
import { CRITICALITIES, SYSTEM_TYPES, humanise } from './constants'

export function NewSystemForm({ onDone }: { onDone: () => void }) {
  const [name, setName] = useState('')
  const [systemType, setSystemType] = useState<SystemType>('APPLICATION')
  const [criticality, setCriticality] = useState<Criticality>('MEDIUM')
  const [description, setDescription] = useState('')
  const queryClient = useQueryClient()

  const create = useMutation({
    mutationFn: () =>
      api.post<System>('/systems', {
        name,
        system_type: systemType,
        criticality,
        description: description || null,
      }),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ['systems'] })
      onDone()
    },
  })

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault()
        if (name.trim()) create.mutate()
      }}
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <label className="block sm:col-span-2">
          <span className="mb-1 block text-xs text-neutral-400">Name</span>
          <input autoFocus className={inputClass} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-neutral-400">Type</span>
          <select
            className={inputClass}
            value={systemType}
            onChange={(e) => setSystemType(e.target.value as SystemType)}
          >
            {SYSTEM_TYPES.map((t) => (
              <option key={t} value={t}>{humanise(t)}</option>
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
        <label className="block sm:col-span-2">
          <span className="mb-1 block text-xs text-neutral-400">Description</span>
          <input className={inputClass} value={description} onChange={(e) => setDescription(e.target.value)} />
        </label>
      </div>

      {create.error && (
        <p className="text-sm text-red-400">
          {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
        </p>
      )}

      <button className={buttonClass} disabled={create.isPending || !name.trim()}>
        {create.isPending ? 'Creating…' : 'Create system'}
      </button>
    </form>
  )
}
