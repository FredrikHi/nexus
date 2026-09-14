import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ApiError } from '../../lib/api'
import type { ApiKey, CreatedApiKey, Environment } from '../../lib/types'
import { Badge, Card, EmptyState, PageHeader } from '../../components/ui'
import { buttonClass, inputClass, relativeTime } from '../../lib/format'

export function ApiKeys() {
  const [name, setName] = useState('')
  const [environmentId, setEnvironmentId] = useState('')
  const [allowAutoCreate, setAllowAutoCreate] = useState(true)
  // Held in component state and never refetched: the server stores only a
  // hash, so this is the one moment the plaintext exists outside the call
  // that created it.
  const [issued, setIssued] = useState<CreatedApiKey | null>(null)
  const [copied, setCopied] = useState(false)

  const queryClient = useQueryClient()

  const keys = useQuery({
    queryKey: ['api-keys'],
    queryFn: () => api.get<ApiKey[]>('/api-keys'),
  })

  const environments = useQuery({
    queryKey: ['environments'],
    queryFn: () => api.get<Environment[]>('/environments'),
  })

  const create = useMutation({
    mutationFn: () =>
      api.post<CreatedApiKey>('/api-keys', {
        name: name.trim(),
        environment_id: environmentId || null,
        allow_auto_create: allowAutoCreate,
      }),
    onSuccess: (key) => {
      setIssued(key)
      setCopied(false)
      setName('')
      setEnvironmentId('')
      void queryClient.invalidateQueries({ queryKey: ['api-keys'] })
    },
  })

  const revoke = useMutation({
    mutationFn: (id: string) => api.delete(`/api-keys/${id}`),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ['api-keys'] }),
  })

  const environmentName = (id: string | null) =>
    id ? (environments.data?.find((e) => e.id === id)?.name ?? 'Unknown') : 'Any'

  return (
    <div>
      <PageHeader
        title="API keys"
        subtitle="Credentials your applications use to send telemetry. They can write events and read nothing."
      />

      {issued && (
        <Card className="mb-6 border-emerald-900 bg-emerald-950/30">
          <p className="text-sm text-emerald-200">
            Copy <span className="font-medium">{issued.name}</span> now. Only a hash of it is
            stored, so it cannot be shown again. Lose it and you create a replacement.
          </p>
          <div className="mt-3 flex flex-wrap items-center gap-2">
            <code className="flex-1 overflow-x-auto rounded border border-emerald-900 bg-neutral-950 px-3 py-2 font-mono text-xs text-emerald-300">
              {issued.token}
            </code>
            <button
              className={buttonClass}
              onClick={() => {
                void navigator.clipboard.writeText(issued.token).then(() => setCopied(true))
              }}
            >
              {copied ? 'Copied' : 'Copy'}
            </button>
            <button
              className="text-xs text-neutral-500 hover:text-neutral-300"
              onClick={() => setIssued(null)}
            >
              Dismiss
            </button>
          </div>
          <p className="mt-3 text-xs text-emerald-200/70">
            Set it in your application as <code className="font-mono">ICC_API_KEY</code>, with{' '}
            <code className="font-mono">ICC_URL</code> pointing at this instance.
          </p>
        </Card>
      )}

      <Card className="mb-6">
        <form
          className="flex flex-wrap items-end gap-3"
          onSubmit={(e) => {
            e.preventDefault()
            if (name.trim()) create.mutate()
          }}
        >
          <label className="flex min-w-48 flex-1 flex-col gap-1">
            <span className="text-xs text-neutral-500">Name</span>
            <input
              className={inputClass}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="cookly-api-production"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs text-neutral-500">Environment</span>
            <select
              className={inputClass}
              value={environmentId}
              onChange={(e) => setEnvironmentId(e.target.value)}
            >
              <option value="">Any</option>
              {environments.data?.map((env) => (
                <option key={env.id} value={env.id}>
                  {env.name}
                </option>
              ))}
            </select>
          </label>

          <label className="flex items-center gap-2 pb-2 text-sm text-neutral-400">
            <input
              type="checkbox"
              checked={allowAutoCreate}
              onChange={(e) => setAllowAutoCreate(e.target.checked)}
            />
            Create unknown integrations
          </label>

          <button className={`${buttonClass} mb-1`} disabled={create.isPending || !name.trim()}>
            Create key
          </button>
        </form>

        <p className="mt-3 text-xs text-neutral-500">
          Pinning a key to an environment stops a staging agent writing events that claim to be
          production. Leaving the checkbox on lets an application report against integrations
          nobody has modelled yet; those arrive under Unmapped for you to wire up. Turn it off
          once your landscape is complete, and a misspelled name becomes an error rather than a
          new integration.
        </p>

        {create.error && (
          <p className="mt-2 text-sm text-red-400">
            {create.error instanceof ApiError ? create.error.message : 'Could not create it.'}
          </p>
        )}
      </Card>

      {keys.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {keys.data?.length === 0 && (
        <EmptyState
          title="No keys yet"
          hint="Create one above, then set it as ICC_API_KEY in the application you want to watch."
        />
      )}

      {keys.data && keys.data.length > 0 && (
        <div className="divide-y divide-neutral-800 rounded-lg border border-neutral-800">
          {keys.data.map((key) => (
            <div key={key.id} className="flex flex-wrap items-center gap-3 p-3">
              <span
                className={
                  key.revoked_at ? 'text-sm text-neutral-600' : 'text-sm text-neutral-100'
                }
              >
                {key.name}
              </span>
              <code className="font-mono text-xs text-neutral-600">{key.prefix}…</code>

              <Badge className="border-neutral-700 bg-neutral-900 text-neutral-400">
                {environmentName(key.environment_id)}
              </Badge>
              {key.allow_auto_create && !key.revoked_at && (
                <Badge className="border-amber-900 bg-amber-950 text-amber-300">
                  Creates integrations
                </Badge>
              )}
              {key.revoked_at && (
                <Badge className="border-red-900 bg-red-950 text-red-300">Revoked</Badge>
              )}

              <span className="text-xs text-neutral-600">
                {key.last_used_at ? `used ${relativeTime(key.last_used_at)}` : 'never used'}
              </span>

              {!key.revoked_at && (
                <button
                  className="ml-auto text-xs text-neutral-600 hover:text-red-400"
                  onClick={() => {
                    const message = `Revoke ${key.name}? Anything still using it stops being able to send telemetry immediately.`
                    if (confirm(message)) revoke.mutate(key.id)
                  }}
                >
                  Revoke
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {revoke.error && (
        <p className="mt-2 text-sm text-red-400">
          {revoke.error instanceof ApiError ? revoke.error.message : 'Could not revoke it.'}
        </p>
      )}
    </div>
  )
}
