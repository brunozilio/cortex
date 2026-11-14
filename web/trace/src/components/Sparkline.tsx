import { Area, AreaChart, YAxis } from 'recharts'
import { ChartContainer, type ChartConfig } from '@/components/ui/chart'

export function Sparkline({ values, color }: { values: number[]; color: string }) {
  const config: ChartConfig = { value: { label: 'value', color } }
  const data = values.map((value, i) => ({ i, value }))
  return (
    <ChartContainer config={config} className="aspect-auto h-8 w-full">
      <AreaChart data={data} margin={{ top: 2, right: 1, bottom: 1, left: 1 }}>
        <YAxis hide domain={[0, (max: number) => max * 1.1 || 1]} />
        <Area
          dataKey="value"
          type="monotone"
          stroke="var(--color-value)"
          strokeWidth={1.5}
          fill="var(--color-value)"
          fillOpacity={0.12}
          isAnimationActive={false}
          dot={false}
        />
      </AreaChart>
    </ChartContainer>
  )
}
