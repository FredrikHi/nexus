import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { Environment } from '../../lib/types'
import { Badge, Card, PageHeader } from '../../components/ui'
import { buttonClass, inputClass } from '../../lib/format'

export function Environments() {
  const [name, setName] = useState('')
  const [isProduction, setIsProduction] = useState(false)
  const queryClient = useQueryClient()

  const environments = useQuery({
    queryKey: ['environments'],
    queryFn: () => api.get<Environment[]>('/environments'),
  })

  const create = useMutation({
    mutationFn: () =>
      api.post<Environment>('/environments', { name, is_production: isProduction }),
    onSuccess: () => {
      setName('')
      setIsProduction(false)
      void queryClient.invalidateQueries({ queryKey: ['environments'] })
    },
  })

  const remove = useMutation({
    // force detaches integrations still pointing at it. The API refuses
    // without it, so the confirm below is where that decision gets made.
    mutationFn: (id: string) => api.delete(`/environments/${id}?force=true`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['environments'] }),
  })

  return (
    <div>
      <PageHeader
        title="Environments"
        subtitle="Where an integration runs. Every organization starts with the standard four; add your own as needed."
      />

      <Card className="mb-6">
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            if (name.trim()) create.mutate()
          }}
        >
          <input
            className={`${inputClass} min-w-48 flex-1`}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="New environment name"
          />
          <label className="flex items-center gap-2 px-1 text-sm text-neutral-400">
            <input
              type="checkbox"
              checked={isProduction}
              onChange={(e) => setIsProduction(e.target.checked)}
            />
            Production
          </label>
          <button className={buttonClass} disabled={create.isPending || !name.trim()}>
            Create
          </button>
        </form>
        {create.error && (
          <p className="mt-2 text-sm text-red-400">
            {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
          </p>
        )}
      </Card>

      {environments.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      <div className="divide-y divide-neutral-800 rounded-lg border border-neutral-800">
        {environments.data?.map((env) => (
          <div key={env.id} className="flex items-center gap-3 p-3">
            <span className="text-sm text-neutral-100">{env.name}</span>
            <span className="font-mono text-xs text-neutral-600">{env.slug}</span>
            {env.is_production && (
              <Badge className="border-red-900 bg-red-950 text-red-300">Production</Badge>
            )}
            <button
              className="ml-auto text-xs text-neutral-600 hover:text-red-400"
              onClick={() => {
                const message =
                  'Delete ' + env.name +
                  '? Integrations using it keep working but lose their environment.'
                if (confirm(message)) remove.mutate(env.id)
              }}
            >
              Delete
            </button>
          </div>
        ))}
      </div>

      {remove.error && (
        <p className="mt-2 text-sm text-red-400">
          {remove.error instanceof ApiError ? remove.error.message : 'Could not delete it.'}
        </p>
      )}
    </div>
  )
}
