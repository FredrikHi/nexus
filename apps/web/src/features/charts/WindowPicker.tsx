import { WINDOWS } from './useSeries'

export function WindowPicker({
  hours,
  onChange,
}: {
  hours: number
  onChange: (hours: number) => void
}) {
  return (
    <div className="flex gap-1">
      {WINDOWS.map((w) => (
        <button
          key={w.label}
          onClick={() => onChange(w.hours)}
          className={`rounded px-2 py-1 text-xs ${
            w.hours === hours
              ? 'bg-neutral-800 text-neutral-100'
              : 'text-neutral-500 hover:text-neutral-300'
          }`}
        >
          {w.label}
        </button>
      ))}
    </div>
  )
}
