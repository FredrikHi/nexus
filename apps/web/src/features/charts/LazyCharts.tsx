import { Suspense, lazy } from 'react'
import type { SeriesPoint } from '../../lib/types'

// The charting library is around 200kB. Loading it with the dashboard would
// put it in the bundle someone downloads before they have even signed in, so
// the charts arrive separately and the page renders without them first.
const VolumeChartImpl = lazy(() =>
  import('./TelemetryChart').then((m) => ({ default: m.VolumeChart })),
)
const ErrorRateChartImpl = lazy(() =>
  import('./TelemetryChart').then((m) => ({ default: m.ErrorRateChart })),
)

/** Reserves the chart's height while it loads, so nothing jumps. */
function ChartFallback() {
  return <div className="h-[200px] animate-pulse rounded bg-neutral-900/60" />
}

interface Props {
  data: SeriesPoint[]
  hours: number
}

export function VolumeChart(props: Props) {
  return (
    <Suspense fallback={<ChartFallback />}>
      <VolumeChartImpl {...props} />
    </Suspense>
  )
}

export function ErrorRateChart(props: Props) {
  return (
    <Suspense fallback={<ChartFallback />}>
      <ErrorRateChartImpl {...props} />
    </Suspense>
  )
}

export { WindowPicker } from './WindowPicker'
