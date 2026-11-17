import { useMemo } from 'react'
import { Bar, BarChart, CartesianGrid, XAxis, YAxis } from 'recharts'
import { Card } from '@/components/ui/card'
import { ChartContainer, ChartLegendContent, ChartTooltip, ChartTooltipContent, type ChartConfig } from '@/components/ui/chart'
import { dailySeries, modelsByCost, periodTotal, PERIOD_DAYS, type Metric } from '@/lib/days'
import { dayLabel, tokens, usd, usdShort } from '@/lib/format'
import { MODEL_COLORS } from '@/lib/kinds'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Report } from '@/types'

const METRICS: Metric[] = ['cost', 'tokens', 'runs']

const TICK_EVERY_DAYS = 5

function formatMetric(metric: Metric, value: number): string {
  if (metric === 'cost') return usd(value)
  if (metric === 'tokens') return tokens(value)
  return String(Math.round(value))
}

function axisValue(metric: Metric, value: number): string {
  if (metric === 'cost') return usdShort(value)
  return formatMetric(metric, value)
}

function MetricToggle({ metric, onMetric }: { metric: Metric; onMetric: (metric: Metric) => void }) {
  const text = useStrings().usage
  return (
    <div role="radiogroup" aria-label={text.metric} className="inline-flex shrink-0 rounded-lg border bg-muted/50 p-0.5">
      {METRICS.map((key) => (
        <button
          key={key}
          type="button"
          role="radio"
          aria-checked={metric === key}
          onClick={() => onMetric(key)}
          className={cn(
            'rounded-md px-3 py-1 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
            metric === key && 'bg-background text-foreground shadow-sm',
          )}
        >
          {text.metrics[key]}
        </button>
      ))}
    </div>
  )
}

interface UsageChartProps {
  report: Report
  metric: Metric
  onMetric: (metric: Metric) => void
}

export function UsageChart({ report, metric, onMetric }: UsageChartProps) {
  const strings = useStrings()
  const text = strings.usage
  const { days } = report.insights
  const models = useMemo(() => modelsByCost(days), [days])
  const config = useMemo<ChartConfig>(
    () => Object.fromEntries(models.map((model, i) => [`m${i}`, { label: model, color: MODEL_COLORS[i % MODEL_COLORS.length] }])),
    [models],
  )
  const data = useMemo(
    () =>
      dailySeries(days, metric, report.generated_at_ms).map((row) => ({
        label: dayLabel(row.day, strings.locale),
        total: row.total,
        ...Object.fromEntries(models.map((model, i) => [`m${i}`, row.values[model] ?? 0])),
      })),
    [days, metric, models, report.generated_at_ms, strings.locale],
  )
  return (
    <Card>
      <div className="flex flex-wrap items-start justify-between gap-3 border-b px-5 py-4">
        <div className="grid gap-1">
          <h3 className="font-semibold leading-none tracking-tight">{text.title}</h3>
          <p className="text-sm text-muted-foreground">
            <span className="num font-medium text-foreground">{formatMetric(metric, periodTotal(days, metric))}</span> {text.nouns[metric]} {text.over(PERIOD_DAYS)}
          </p>
        </div>
        <MetricToggle metric={metric} onMetric={onMetric} />
      </div>
      <div className="p-4 sm:p-5">
        <ChartContainer config={config} className="aspect-auto h-[260px] w-full [&_.recharts-cartesian-axis-tick_text]:tabular-nums">
          <BarChart data={data} margin={{ top: 8, right: 8, left: 0, bottom: 0 }} barCategoryGap="22%">
            <CartesianGrid vertical={false} />
            <XAxis dataKey="label" tickLine={false} axisLine={false} tickMargin={8} interval={TICK_EVERY_DAYS - 1} />
            <YAxis tickLine={false} axisLine={false} width={52} tickFormatter={(value: number) => axisValue(metric, value)} />
            <ChartTooltip
              cursor={{ fill: 'hsl(var(--muted))', fillOpacity: 0.6 }}
              content={
                <ChartTooltipContent
                  valueFormatter={(value) => formatMetric(metric, value)}
                  footer={(payload) => (
                    <div className="mt-1 flex border-t pt-1.5">
                      <span className="text-muted-foreground">{text.total}</span>
                      <span className="ml-auto font-mono font-medium">{formatMetric(metric, Number(payload[0]?.payload?.total ?? 0))}</span>
                    </div>
                  )}
                />
              }
            />
            {models.map((_, i) => (
              <Bar key={i} dataKey={`m${i}`} stackId="usage" fill={`var(--color-m${i})`} isAnimationActive={false} />
            ))}
          </BarChart>
        </ChartContainer>
        <ChartLegendContent config={config} className="mt-2" />
      </div>
    </Card>
  )
}
