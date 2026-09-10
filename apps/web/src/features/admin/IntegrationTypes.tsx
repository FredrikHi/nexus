import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { IntegrationType } from '../../lib/types'
import { Badge, Card, PageHeader } from '../../components/ui'
import { buttonClass, inputClass } from '../../lib/format'

export function IntegrationTypes() {
  const [name, setName] = useState('')
  const queryClient = useQueryClient()

  const types = useQuery({
    queryKey: ['integration-types'],
    queryFn: () => api.get<IntegrationType[]>('/integration-types'),
  })

  const create = useMutation({
    mutationFn: () => api.post<IntegrationType>('/integration-types', { name }),
    onSuccess: () => {
      setName('')
      void queryClient.invalidateQueries({ queryKey: ['integration-types'] })
    },
  })

  const remove = useMutation({
    mutationFn: (id: string) => api.delete(`/integration-types/${id}`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['integration-types'] }),
  })

  const builtins = types.data?.filter((t) => t.is_builtin) ?? []
  const custom = types.data?.filter((t) => !t.is_builtin) ?? []

  return (
    <div>
      <PageHeader
        title="Integration types"
        subtitle="What kind of thing an integration is. The built-ins are shared; anything you add is visible only to this organization."
      />

      <Card className="mb-6">
        <form
          className="flex gap-2"
          onSubmit={(e) => {
            e.preventDefault()
            if (name.trim()) create.mutate()
          }}
        >
          <input
            className={inputClass}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g. Peppol BIS"
          />
          <button className={buttonClass} disabled={create.isPending || !name.trim()}>
            Add type
          </button>
        </form>
        <p className="mt-2 text-xs text-neutral-600">
          The key is derived from the name, so Peppol BIS becomes PEPPOL_BIS.
        </p>
        {create.error && (
          <p className="mt-2 text-sm text-red-400">
            {create.error instanceof ApiError ? create.error.message : 'Could not add it.'}
          </p>
        )}
      </Card>

      {custom.length > 0 && (
        <section className="mb-6">
          <h2 className="mb-2 text-sm font-medium text-neutral-300">Your types</h2>
          <div className="divide-y divide-neutral-800 rounded-lg border border-neutral-800">
            {custom.map((t) => (
              <div key={t.id} className="flex items-center gap-3 p-3">
                <span className="text-sm text-neutral-100">{t.name}</span>
                <span className="font-mono text-xs text-neutral-600">{t.key}</span>
                <button
                  className="ml-auto text-xs text-neutral-600 hover:text-red-400"
                  onClick={() => remove.mutate(t.id)}
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
        </section>
      )}

      <section>
        <h2 className="mb-2 text-sm font-medium text-neutral-300">Built in</h2>
        <div className="flex flex-wrap gap-2">
          {builtins.map((t) => (
            <Badge key={t.id} className="border-neutral-700 bg-neutral-800 text-neutral-400">
              {t.name}
            </Badge>
          ))}
        </div>
        <p className="mt-2 text-xs text-neutral-600">
          Built-in types ship with the platform and cannot be edited or removed.
        </p>
      </section>
    </div>
  )
}
