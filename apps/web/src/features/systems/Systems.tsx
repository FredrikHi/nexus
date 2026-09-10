import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '../../lib/api'
import type { System } from '../../lib/types'
import { Badge, Card, EmptyState, PageHeader } from '../../components/ui'
import { buttonClass } from '../../lib/format'
import { CRITICALITY_STYLES, humanise } from './constants'
import { ComponentsPanel } from './ComponentsPanel'
import { NewSystemForm } from './NewSystemForm'

export function Systems() {
  const [adding, setAdding] = useState(false)
  const systems = useQuery({
    queryKey: ['systems'],
    queryFn: () => api.get<System[]>('/systems'),
  })

  return (
    <div>
      <PageHeader
        title="Systems"
        subtitle="A deployable unit or an external service. Third-party services are systems too."
        action={
          <button className={buttonClass} onClick={() => setAdding(!adding)}>
            {adding ? 'Cancel' : 'Add system'}
          </button>
        }
      />

      {adding && (
        <Card className="mb-6">
          <NewSystemForm onDone={() => setAdding(false)} />
        </Card>
      )}

      {systems.isPending && <p className="text-sm text-neutral-500">Loading…</p>}

      {systems.data?.length === 0 && !adding && (
        <EmptyState
          title="No systems yet."
          hint="Add the applications, databases and third-party services you integrate with."
        />
      )}

      <div className="space-y-3">
        {systems.data?.map((system) => (
          <SystemRow key={system.id} system={system} />
        ))}
      </div>
    </div>
  )
}

function SystemRow({ system }: { system: System }) {
  const [open, setOpen] = useState(false)

  return (
    <Card>
      <button className="flex w-full items-start gap-3 text-left" onClick={() => setOpen(!open)}>
        <span className="mt-1 text-neutral-600">{open ? '▾' : '▸'}</span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-sm font-medium text-neutral-100">{system.name}</span>
            <Badge className="border-neutral-700 bg-neutral-800 text-neutral-400">
              {humanise(system.system_type)}
            </Badge>
            <Badge className={CRITICALITY_STYLES[system.criticality]}>
              {humanise(system.criticality)}
            </Badge>
          </div>
          {system.description && (
            <p className="mt-1 text-sm text-neutral-500">{system.description}</p>
          )}
        </div>
      </button>

      {/* Mounted only when expanded, so the components query runs on demand. */}
      {open && (
        <div className="mt-4 border-t border-neutral-800 pt-4 pl-6">
          <ComponentsPanel systemId={system.id} />
        </div>
      )}
    </Card>
  )
}
