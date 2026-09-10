import type { Criticality, SystemType, ComponentType } from '../../lib/types'

export const SYSTEM_TYPES: SystemType[] = [
  'APPLICATION', 'API', 'DATABASE', 'EXTERNAL_SERVICE',
  'MESSAGE_BROKER', 'IDENTITY_PROVIDER', 'FILE_SERVICE', 'OTHER',
]

export const COMPONENT_TYPES: ComponentType[] = [
  'FRONTEND', 'API', 'CONTROLLER', 'SERVICE', 'REPOSITORY', 'DATABASE',
  'STORED_PROCEDURE', 'WORKER', 'JOB', 'QUEUE', 'WEBHOOK', 'OTHER',
]

export const CRITICALITIES: Criticality[] = ['LOW', 'MEDIUM', 'HIGH', 'CRITICAL']

export const CRITICALITY_STYLES: Record<Criticality, string> = {
  LOW: 'bg-neutral-800 text-neutral-400 border-neutral-700',
  MEDIUM: 'bg-neutral-800 text-neutral-300 border-neutral-700',
  HIGH: 'bg-amber-950 text-amber-300 border-amber-900',
  CRITICAL: 'bg-red-950 text-red-300 border-red-900',
}

/** SCREAMING_SNAKE reads badly in a UI; the API keeps the tokens. */
export const humanise = (token: string) =>
  token.charAt(0) + token.slice(1).toLowerCase().replace(/_/g, ' ')
