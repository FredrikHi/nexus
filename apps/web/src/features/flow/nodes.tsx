import { Handle, Position, type NodeProps, type Node } from '@xyflow/react'
import type { FlowNodeData } from './layout'

type FlowNode = Node<FlowNodeData>

/** A system: a labelled container its components sit inside. */
export function SystemNode({ data }: NodeProps<FlowNode>) {
  return (
    <div className="h-full w-full rounded-lg border border-neutral-700 bg-neutral-900/70">
      <div className="border-b border-neutral-800 px-3 py-2">
        <div className="truncate text-sm font-medium text-neutral-100">{data.label}</div>
        <div className="truncate text-[10px] uppercase tracking-wide text-neutral-500">
          {data.sublabel}
        </div>
      </div>
    </div>
  )
}

/** A component: what integrations actually connect. */
export function ComponentNode({ data }: NodeProps<FlowNode>) {
  return (
    <div className="flex h-full w-full items-center rounded border border-neutral-700 bg-neutral-800 px-2">
      {/* Handles on both sides so an edge can enter and leave any component. */}
      <Handle type="target" position={Position.Left} className="!h-2 !w-2 !border-none !bg-neutral-500" />
      <div className="min-w-0">
        <div className="truncate text-xs text-neutral-100">{data.label}</div>
        <div className="truncate text-[10px] text-neutral-500">{data.sublabel}</div>
      </div>
      <Handle type="source" position={Position.Right} className="!h-2 !w-2 !border-none !bg-neutral-500" />
    </div>
  )
}
