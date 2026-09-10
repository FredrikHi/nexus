import type {
  Component, Integration, IntegrationHealth, System,
} from '../../lib/types'

export interface FlowNodeData extends Record<string, unknown> {
  label: string
  sublabel: string
  kind: 'system' | 'component'
  health: IntegrationHealth['status'] | null
}

export interface BuiltGraph {
  nodes: {
    id: string
    position: { x: number; y: number }
    data: FlowNodeData
    type: string
    parentId?: string
    extent?: 'parent'
    style?: Record<string, string | number>
  }[]
  edges: {
    id: string
    source: string
    target: string
    label: string
    data: { health: IntegrationHealth['status'] | null; integrationId: string }
    animated: boolean
  }[]
}

// Laid out by hand rather than with a graph library. The shape here is known:
// systems are columns of components, and integrations are edges between them.
// A force-directed layout would move things every render, which makes a
// landscape you are trying to learn impossible to hold in your head.
const SYSTEM_WIDTH = 260
const SYSTEM_GAP = 120
const COMPONENT_HEIGHT = 52
const COMPONENT_GAP = 12
const HEADER_HEIGHT = 46
const PADDING = 16

/**
 * Builds a graph of systems containing components, connected by integrations.
 *
 * Components are child nodes of their system, so dragging a system moves its
 * contents with it. Systems are placed in a row, widest first, which keeps the
 * busiest parts of the landscape near the left where the eye starts.
 */
export function buildGraph(
  systems: System[],
  components: Component[],
  integrations: Integration[],
  health: IntegrationHealth[],
): BuiltGraph {
  const nodes: BuiltGraph['nodes'] = []
  const edges: BuiltGraph['edges'] = []

  const componentsBySystem = new Map<string, Component[]>()
  for (const component of components) {
    const list = componentsBySystem.get(component.system_id) ?? []
    list.push(component)
    componentsBySystem.set(component.system_id, list)
  }

  // Only systems that have something to show; an empty box teaches nothing.
  const ordered = [...systems].sort(
    (a, b) =>
      (componentsBySystem.get(b.id)?.length ?? 0) - (componentsBySystem.get(a.id)?.length ?? 0) ||
      a.name.localeCompare(b.name),
  )

  ordered.forEach((system, index) => {
    const own = componentsBySystem.get(system.id) ?? []
    const height =
      HEADER_HEIGHT + PADDING + Math.max(1, own.length) * (COMPONENT_HEIGHT + COMPONENT_GAP)

    nodes.push({
      id: system.id,
      type: 'system',
      position: { x: index * (SYSTEM_WIDTH + SYSTEM_GAP), y: 0 },
      data: {
        label: system.name,
        sublabel: system.system_type.replace(/_/g, ' ').toLowerCase(),
        kind: 'system',
        health: null,
      },
      style: { width: SYSTEM_WIDTH, height },
    })

    own.forEach((component, row) => {
      nodes.push({
        id: component.id,
        type: 'component',
        parentId: system.id,
        extent: 'parent',
        position: { x: PADDING, y: HEADER_HEIGHT + row * (COMPONENT_HEIGHT + COMPONENT_GAP) },
        data: {
          label: component.name,
          sublabel: component.component_type.replace(/_/g, ' ').toLowerCase(),
          kind: 'component',
          health: null,
        },
        style: { width: SYSTEM_WIDTH - PADDING * 2, height: COMPONENT_HEIGHT },
      })
    })
  })

  const healthByIntegration = new Map(health.map((h) => [h.integration_id, h.status]))
  const known = new Set(components.map((c) => c.id))

  for (const integration of integrations) {
    // An integration can outlive a component the caller cannot see; skip it
    // rather than drawing an edge into nothing.
    if (!known.has(integration.source_component_id)) continue
    if (!known.has(integration.destination_component_id)) continue

    const status = healthByIntegration.get(integration.id) ?? null
    edges.push({
      id: integration.id,
      source: integration.source_component_id,
      target: integration.destination_component_id,
      label: integration.name,
      data: { health: status, integrationId: integration.id },
      // Only trouble moves. Animating everything makes nothing stand out.
      animated: status === 'UNHEALTHY' || status === 'DEGRADED',
    })
  }

  return { nodes, edges }
}
