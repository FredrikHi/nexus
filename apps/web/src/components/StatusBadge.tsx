import type { TelemetryStatus } from '../lib/types'
import { Badge } from './ui'

// Telemetry statuses reuse the health palette so a red thing always means the
// same kind of trouble, wherever it appears.
const STYLES: Record<TelemetryStatus, string> = {
  SUCCESS: 'bg-emerald-950 text-emerald-300 border-emerald-900',
  FAILURE: 'bg-red-950 text-red-300 border-red-900',
  TIMEOUT: 'bg-orange-950 text-orange-300 border-orange-900',
  REJECTED: 'bg-neutral-800 text-neutral-400 border-neutral-700',
}

export function TelemetryStatusBadge({ status }: { status: TelemetryStatus }) {
  return <Badge className={STYLES[status]}>{status}</Badge>
}
