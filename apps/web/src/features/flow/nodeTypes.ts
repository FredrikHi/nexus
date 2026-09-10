import { ComponentNode, SystemNode } from './nodes'

// A plain lookup rather than an export from the component module, so fast
// refresh keeps working there.
export const NODE_TYPES = { system: SystemNode, component: ComponentNode }
