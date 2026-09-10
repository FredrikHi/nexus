// Formatting helpers and shared class strings.
//
// Kept out of the component file so fast refresh works there: a module that
// exports both components and plain values cannot be hot-reloaded cleanly.

/** "3m ago". Precision beyond this is noise for an operator. */
export function relativeTime(iso: string | null): string {
  if (!iso) return 'never'
  const seconds = Math.round((Date.now() - new Date(iso).getTime()) / 1000)
  if (seconds < 60) return 'just now'
  const minutes = Math.round(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  return `${Math.round(hours / 24)}d ago`
}

export function formatMs(value: number | null): string {
  if (value === null) return '-'
  if (value < 1000) return `${Math.round(value)}ms`
  return `${(value / 1000).toFixed(1)}s`
}

export function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`
}

export const inputClass =
  'w-full rounded border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm text-neutral-100 outline-none focus:border-neutral-500'

export const buttonClass =
  'rounded bg-neutral-100 px-3 py-1.5 text-sm font-medium text-neutral-900 hover:bg-white disabled:opacity-50'
