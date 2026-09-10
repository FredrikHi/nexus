import type { TelemetryStatus } from './types'

// Chart colours matched to the badge palette, so a series and its legend agree
// wherever they appear. Kept out of the badge module so fast refresh works.
export const STATUS_COLORS: Record<TelemetryStatus, string> = {
  SUCCESS: '#34d399',
  FAILURE: '#f87171',
  TIMEOUT: '#fb923c',
  REJECTED: '#a3a3a3',
}
