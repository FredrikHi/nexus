import {
  Area, AreaChart, CartesianGrid, Line, LineChart, ResponsiveContainer,
  Tooltip, XAxis, YAxis,
} from 'recharts'
import type { SeriesPoint } from '../../lib/types'
import { STATUS_COLORS } from '../../lib/statusColors'
import { formatMs } from '../../lib/format'

/** Formats a bucket for an axis: time of day, or a date once the window is wide. */
function tickLabel(iso: string, hours: number): string {
  const d = new Date(iso)
  if (hours > 48) return `${d.getDate()}/${d.getMonth() + 1}`
  return d.toTimeString().slice(0, 5)
}

const AXIS = { stroke: '#525252', fontSize: 11 }
const TOOLTIP_STYLE = {
  background: '#171717',
  border: '1px solid #404040',
  borderRadius: 6,
  fontSize: 12,
}

/** Stacked volume by outcome. Answers "how much traffic, and how did it go". */
export function VolumeChart({ data, hours }: { data: SeriesPoint[]; hours: number }) {
  return (
    <ResponsiveContainer width="100%" height={200}>
      <AreaChart data={data} margin={{ top: 4, right: 8, bottom: 0, left: -18 }}>
        <CartesianGrid stroke="#262626" vertical={false} />
        <XAxis dataKey="bucket" tickFormatter={(v: string) => tickLabel(v, hours)} {...AXIS} />
        <YAxis {...AXIS} allowDecimals={false} />
        <Tooltip
          contentStyle={TOOLTIP_STYLE}
          labelFormatter={(label) => new Date(String(label)).toLocaleString()}
        />
        {/* Stacked in severity order so the eye reads trouble at the top. */}
        <Area type="monotone" dataKey="success" stackId="1" name="Success"
              stroke={STATUS_COLORS.SUCCESS} fill={STATUS_COLORS.SUCCESS} fillOpacity={0.25} />
        <Area type="monotone" dataKey="rejected" stackId="1" name="Rejected"
              stroke={STATUS_COLORS.REJECTED} fill={STATUS_COLORS.REJECTED} fillOpacity={0.25} />
        <Area type="monotone" dataKey="timeout" stackId="1" name="Timeout"
              stroke={STATUS_COLORS.TIMEOUT} fill={STATUS_COLORS.TIMEOUT} fillOpacity={0.35} />
        <Area type="monotone" dataKey="failure" stackId="1" name="Failure"
              stroke={STATUS_COLORS.FAILURE} fill={STATUS_COLORS.FAILURE} fillOpacity={0.35} />
      </AreaChart>
    </ResponsiveContainer>
  )
}

/** Error rate against latency: the two numbers health is judged on. */
export function ErrorRateChart({ data, hours }: { data: SeriesPoint[]; hours: number }) {
  const shaped = data.map((p) => ({ ...p, error_percent: p.error_rate * 100 }))

  return (
    <ResponsiveContainer width="100%" height={200}>
      <LineChart data={shaped} margin={{ top: 4, right: 8, bottom: 0, left: -18 }}>
        <CartesianGrid stroke="#262626" vertical={false} />
        <XAxis dataKey="bucket" tickFormatter={(v: string) => tickLabel(v, hours)} {...AXIS} />
        {/* Two axes because a percentage and a duration share no scale. */}
        <YAxis yAxisId="left" {...AXIS} unit="%" domain={[0, 100]} />
        <YAxis yAxisId="right" orientation="right" {...AXIS} />
        <Tooltip
          contentStyle={TOOLTIP_STYLE}
          labelFormatter={(label) => new Date(String(label)).toLocaleString()}
          formatter={(value, name) => {
            const n = typeof value === 'number' ? value : Number(value)
            if (!Number.isFinite(n)) return '-'
            return name === 'p95' ? formatMs(n) : `${n.toFixed(1)}%`
          }}
        />
        <Line yAxisId="left" type="monotone" dataKey="error_percent" name="Error rate"
              stroke={STATUS_COLORS.FAILURE} dot={false} strokeWidth={2} />
        <Line yAxisId="right" type="monotone" dataKey="p95_duration_ms" name="p95"
              stroke="#60a5fa" dot={false} strokeWidth={2} connectNulls />
      </LineChart>
    </ResponsiveContainer>
  )
}
