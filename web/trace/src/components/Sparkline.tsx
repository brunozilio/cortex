import { useId } from 'react'
import { Area, AreaChart, YAxis } from 'recharts'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'

export function Sparkline({ values, color }: { values: number[]; color: string }) {
  const gradient = `spark-${useId().replace(/:/g, '')}`
  const config: ChartConfig = { value: { label: 'value', color } }
  const data = values.map((value, i) => ({ i, value }))
  return (
    <ChartContainer config={config} className="aspect-auto h-9 w-full">
      <AreaChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: 4 }}>
        <defs>
          <linearGradient id={gradient} x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="var(--color-value)" stopOpacity={0.35} />
            <stop offset="1" stopColor="var(--color-value)" stopOpacity={0} />
          </linearGradient>
        </defs>
        <YAxis hide domain={[0, (max: number) => max * 1.15 || 1]} />
        <Area dataKey="value" type="monotone" stroke="var(--color-value)" strokeWidth={1.75} fill={`url(#${gradient})`} isAnimationActive={false} dot={false} />
      </AreaChart>
    </ChartContainer>
  )
}
