import { useMemo } from 'react'
import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from 'recharts'
import { Card } from '@/components/ui/card'
import { ChartContainer, ChartLegendContent, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart'
import { dailySeries, modelsByCost, periodTotal, type Metric } from '@/lib/days'
import { dayLabel, tokens, usd, usdShort } from '@/lib/format'
import { MODEL_COLORS } from '@/lib/kinds'
import { cn } from '@/lib/utils'
import type { Report } from '@/types'

const METRICS: { key: Metric; label: string }[] = [
  { key: 'cost', label: 'Cost' },
  { key: 'tokens', label: 'Tokens' },
  { key: 'runs', label: 'Runs' },
]

function formatMetric(metric: Metric, value: number): string {
  if (metric === 'cost') return usd(value)
  if (metric === 'tokens') return tokens(value)
  return String(Math.round(value))
}

function axisValue(metric: Metric, value: number): string {
  if (metric === 'cost') return usdShort(value)
  return formatMetric(metric, value)
}

interface UsageChartProps {
  report: Report
  metric: Metric
  onMetric: (metric: Metric) => void
}

export function UsageChart({ report, metric, onMetric }: UsageChartProps) {
  const { days } = report.insights
  const models = useMemo(() => modelsByCost(days), [days])
  const config = useMemo<ChartConfig>(
    () => Object.fromEntries(models.map((model, i) => [`m${i}`, { label: model, color: MODEL_COLORS[i % MODEL_COLORS.length] }])),
    [models],
  )
  const data = useMemo(
    () =>
      dailySeries(days, metric, report.generated_at_ms).map((row) => ({
        label: dayLabel(row.day),
        total: row.total,
        ...Object.fromEntries(models.map((model, i) => [`m${i}`, row.values[model] ?? 0])),
      })),
    [days, metric, models, report.generated_at_ms],
  )
  return (
    <Card>
      <div className="flex flex-col items-stretch border-b sm:flex-row">
        <div className="flex flex-1 flex-col justify-center gap-1 px-5 py-4">
          <h3 className="font-semibold leading-none tracking-tight">Usage by model</h3>
          <p className="text-sm text-muted-foreground">Daily, stacked. Hover a day for the split.</p>
        </div>
        <div className="flex">
          {METRICS.map(({ key, label }) => (
            <button
              key={key}
              type="button"
              aria-pressed={metric === key}
              onClick={() => onMetric(key)}
              className={cn(
                'flex min-w-[110px] flex-1 flex-col gap-1 border-t px-5 py-4 text-left hover:bg-muted/40 sm:border-l sm:border-t-0',
                metric === key && 'bg-muted/60',
              )}
            >
              <span className="text-xs text-muted-foreground">{label}</span>
              <span className="num text-lg font-semibold leading-none sm:text-2xl">
                {key === 'cost' ? usdShort(periodTotal(days, key)) : formatMetric(key, periodTotal(days, key))}
              </span>
            </button>
          ))}
        </div>
      </div>
      <div className="p-4 sm:p-5">
        <ChartContainer config={config} className="aspect-auto h-[260px] w-full">
          <AreaChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
            <defs>
              {models.map((_, i) => (
                <linearGradient key={i} id={`usage-fill-m${i}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0" stopColor={`var(--color-m${i})`} stopOpacity={0.55} />
                  <stop offset="1" stopColor={`var(--color-m${i})`} stopOpacity={0.08} />
                </linearGradient>
              ))}
            </defs>
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={8} minTickGap={32} />
            <YAxis tickLine={false} axisLine={false} width={52} tickFormatter={(value: number) => axisValue(metric, value)} className="font-mono" />
            <ChartTooltip
              cursor={{ strokeDasharray: '4 3' }}
              content={
                <ChartTooltipContent
                  valueFormatter={(value) => formatMetric(metric, value)}
                  footer={(payload) => (
                    <div className="mt-1 flex border-t pt-1.5">
                      <span className="text-muted-foreground">Total</span>
                      <span className="ml-auto font-mono font-medium">{formatMetric(metric, Number(payload[0]?.payload?.total ?? 0))}</span>
                    </div>
                  )}
                />
              }
            />
            {models.map((_, i) => (
              <Area
                key={i}
                dataKey={`m${i}`}
                type="monotone"
                stackId="usage"
                stroke={`var(--color-m${i})`}
                strokeWidth={1.75}
                fill={`url(#usage-fill-m${i})`}
                isAnimationActive={false}
              />
            ))}
          </AreaChart>
        </ChartContainer>
        <ChartLegendContent config={config} className="mt-2" />
      </div>
    </Card>
  )
}
