import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  Background, Controls, MiniMap, ReactFlow, type Edge, type Node,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { api } from '../../lib/api'
import type {
  Component, HealthStatus, Integration, IntegrationHealth, System,
} from '../../lib/types'
import { EmptyState, HealthBadge, PageHeader } from '../../components/ui'
import { buildGraph, type FlowNodeData } from './layout'
import { NODE_TYPES } from './nodeTypes'

// Edge colour is the only signal that scales: at thirty integrations you
// cannot read labels, but you can see where the red is.
const EDGE_COLORS: Record<HealthStatus, string> = {
  HEALTHY: '#34d399',
  DEGRADED: '#fbbf24',
  UNHEALTHY: '#f87171',
  UNKNOWN: '#525252',
}

export function Flow() {
  const [selected, setSelected] = useState<string | null>(null)

  const systems = useQuery({ queryKey: ['systems'], queryFn: () => api.get<System[]>('/systems') })
  const components = useQuery({
    queryKey: ['components'],
    queryFn: () => api.get<Component[]>('/components'),
  })
  const integrations = useQuery({
    queryKey: ['integrations'],
    queryFn: () => api.get<Integration[]>('/integrations'),
  })
  const health = useQuery({
    queryKey: ['integration-health'],
    queryFn: () => api.get<IntegrationHealth[]>('/integration-health'),
    refetchInterval: 30_000,
  })

  const graph = useMemo(
    () =>
      buildGraph(
        systems.data ?? [],
        components.data ?? [],
        integrations.data ?? [],
        health.data ?? [],
      ),
    [systems.data, components.data, integrations.data, health.data],
  )

  const nodes = graph.nodes as unknown as Node<FlowNodeData>[]
  const edges: Edge[] = graph.edges.map((e) => {
    const color = EDGE_COLORS[e.data.health ?? 'UNKNOWN']
    return {
      ...e,
      style: { stroke: color, strokeWidth: e.data.health === 'HEALTHY' ? 1.5 : 2.5 },
      labelStyle: { fill: '#d4d4d4', fontSize: 10 },
      labelBgStyle: { fill: '#171717' },
      labelBgPadding: [4, 2] as [number, number],
      labelBgBorderRadius: 3,
    }
  })

  const loading =
    systems.isPending || components.isPending || integrations.isPending

  const selectedIntegration = integrations.data?.find((i) => i.id === selected)
  const selectedHealth = health.data?.find((h) => h.integration_id === selected)

  return (
    <div className="flex h-full flex-col">
      <PageHeader
        title="Flow"
        subtitle="Every system, the components inside it, and the integrations between them. Edge colour is current health."
      />

      {loading && <p className="text-sm text-neutral-500">Loading…</p>}

      {!loading && nodes.length === 0 && (
        <EmptyState
          title="Nothing to draw yet."
          hint="Add systems and components, then connect them, and the map appears here."
        />
      )}

      {nodes.length > 0 && (
        <div className="relative min-h-[520px] flex-1 overflow-hidden rounded-lg border border-neutral-800">
          <ReactFlow
            nodes={nodes}
            edges={edges}
            nodeTypes={NODE_TYPES}
            fitView
            minZoom={0.2}
            proOptions={{ hideAttribution: true }}
            onEdgeClick={(_, edge) => setSelected(edge.id === selected ? null : edge.id)}
            onPaneClick={() => setSelected(null)}
          >
            <Background color="#262626" gap={20} />
            <Controls className="!border-neutral-700 !bg-neutral-900" />
            <MiniMap
              pannable
              className="!bg-neutral-900"
              maskColor="rgba(10,10,10,0.7)"
              nodeColor="#404040"
            />
          </ReactFlow>

          <div className="pointer-events-none absolute left-3 top-3 flex gap-3 rounded border border-neutral-800 bg-neutral-950/80 px-3 py-2 text-[10px] uppercase tracking-wide">
            {(Object.keys(EDGE_COLORS) as HealthStatus[]).map((status) => (
              <span key={status} className="flex items-center gap-1.5 text-neutral-400">
                <span
                  className="inline-block h-0.5 w-4"
                  style={{ background: EDGE_COLORS[status] }}
                />
                {status}
              </span>
            ))}
          </div>

          {selectedIntegration && (
            <div className="absolute bottom-3 left-3 max-w-md rounded-lg border border-neutral-700 bg-neutral-900 p-3">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-neutral-100">
                  {selectedIntegration.name}
                </span>
                {selectedHealth && <HealthBadge status={selectedHealth.status} />}
              </div>
              <p className="mt-1 text-xs text-neutral-400">
                {selectedHealth?.reason ?? 'No telemetry recorded yet.'}
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
