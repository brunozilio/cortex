interface RingProps {
  ratio: number
  color: string
  trackColor?: string
  size?: number
  stroke?: number
  label?: string
}

export function Ring({ ratio, color, trackColor = 'hsl(var(--muted))', size = 72, stroke = 7, label }: RingProps) {
  const radius = size / 2 - stroke + 1
  const circumference = 2 * Math.PI * radius
  const clamped = Math.min(1, Math.max(0, ratio))
  return (
    <div className="relative grid shrink-0 place-items-center" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
        <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke={trackColor} strokeWidth={stroke} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={stroke}
          strokeDasharray={`${circumference * clamped} ${circumference}`}
        />
      </svg>
      {label === '' ? null : <span className="num absolute text-sm font-semibold">{label ?? `${Math.round(clamped * 100)}%`}</span>}
    </div>
  )
}
