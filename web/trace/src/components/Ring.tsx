interface RingProps {
  ratio: number
  color: string
  size?: number
  label?: string
}

export function Ring({ ratio, color, size = 72, label }: RingProps) {
  const radius = size / 2 - 6
  const circumference = 2 * Math.PI * radius
  const clamped = Math.min(1, Math.max(0, ratio))
  return (
    <div className="relative grid shrink-0 place-items-center" style={{ width: size, height: size }}>
      <svg width={size} height={size} className="-rotate-90" aria-hidden="true">
        <circle cx={size / 2} cy={size / 2} r={radius} fill="none" stroke="hsl(var(--muted))" strokeWidth={7} />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={7}
          strokeLinecap="round"
          strokeDasharray={`${circumference * clamped} ${circumference}`}
        />
      </svg>
      <span className="num absolute text-sm font-semibold">{label ?? `${Math.round(clamped * 100)}%`}</span>
    </div>
  )
}
