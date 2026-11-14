import * as React from 'react'
import * as RechartsPrimitive from 'recharts'
import { cn } from '@/lib/utils'

export type ChartConfig = {
  [key: string]: { label?: React.ReactNode; color?: string }
}

type ChartContextProps = { config: ChartConfig }

const ChartContext = React.createContext<ChartContextProps | null>(null)

function useChart() {
  const context = React.useContext(ChartContext)
  if (!context) throw new Error('useChart must be used within a <ChartContainer />')
  return context
}

const ChartContainer = React.forwardRef<
  HTMLDivElement,
  React.ComponentProps<'div'> & {
    config: ChartConfig
    children: React.ComponentProps<typeof RechartsPrimitive.ResponsiveContainer>['children']
  }
>(({ id, className, children, config, ...props }, ref) => {
  const uniqueId = React.useId()
  const chartId = `chart-${id || uniqueId.replace(/:/g, '')}`
  return (
    <ChartContext.Provider value={{ config }}>
      <div
        data-chart={chartId}
        ref={ref}
        className={cn(
          "flex aspect-video justify-center text-xs [&_.recharts-cartesian-axis-tick_text]:fill-muted-foreground [&_.recharts-cartesian-grid_line[stroke='#ccc']]:stroke-border/70 [&_.recharts-curve.recharts-tooltip-cursor]:stroke-border [&_.recharts-dot[stroke='#fff']]:stroke-transparent [&_.recharts-layer]:outline-none [&_.recharts-surface]:outline-none",
          className,
        )}
        {...props}
      >
        <ChartStyle id={chartId} config={config} />
        <RechartsPrimitive.ResponsiveContainer>{children}</RechartsPrimitive.ResponsiveContainer>
      </div>
    </ChartContext.Provider>
  )
})
ChartContainer.displayName = 'Chart'

function ChartStyle({ id, config }: { id: string; config: ChartConfig }) {
  const entries = Object.entries(config).filter(([, item]) => item.color)
  if (!entries.length) return null
  const rules = entries.map(([key, item]) => `  --color-${key}: ${item.color};`).join('\n')
  return <style dangerouslySetInnerHTML={{ __html: `[data-chart=${id}] {\n${rules}\n}` }} />
}

const ChartTooltip = RechartsPrimitive.Tooltip

type TooltipPayloadItem = {
  dataKey?: string | number
  name?: string | number
  value?: number | string | Array<number | string>
  color?: string
  payload?: Record<string, unknown>
}

interface ChartTooltipContentProps extends React.ComponentProps<'div'> {
  active?: boolean
  payload?: TooltipPayloadItem[]
  label?: React.ReactNode
  labelFormatter?: (label: React.ReactNode, payload: TooltipPayloadItem[]) => React.ReactNode
  valueFormatter?: (value: number) => string
  footer?: (payload: TooltipPayloadItem[]) => React.ReactNode
  hideLabel?: boolean
}

const ChartTooltipContent = React.forwardRef<HTMLDivElement, ChartTooltipContentProps>(
  ({ active, payload, label, labelFormatter, valueFormatter, footer, hideLabel, className }, ref) => {
    const { config } = useChart()
    if (!active || !payload?.length) return null
    const format = valueFormatter ?? ((value: number) => value.toLocaleString())
    const items = [...payload].reverse()
    return (
      <div ref={ref} className={cn('grid min-w-44 gap-1.5 rounded-lg border bg-popover px-3 py-2 text-xs shadow-xl', className)}>
        {hideLabel ? null : <div className="font-medium">{labelFormatter ? labelFormatter(label, payload) : label}</div>}
        <div className="grid gap-1">
          {items.map((item) => {
            const key = String(item.dataKey ?? item.name ?? 'value')
            const entry = config[key]
            return (
              <div key={key} className="flex items-center gap-2">
                <span className="size-2.5 shrink-0 rounded-[3px]" style={{ background: item.color ?? `var(--color-${key})` }} />
                <span className="text-muted-foreground">{entry?.label ?? key}</span>
                <span className="ml-auto pl-4 font-mono font-medium tabular-nums">{format(Number(item.value ?? 0))}</span>
              </div>
            )
          })}
        </div>
        {footer ? footer(payload) : null}
      </div>
    )
  },
)
ChartTooltipContent.displayName = 'ChartTooltip'

function ChartLegendContent({ config, className }: { config: ChartConfig; className?: string }) {
  return (
    <div className={cn('flex flex-wrap items-center justify-center gap-4 text-xs text-muted-foreground', className)}>
      {Object.entries(config).map(([key, item]) => (
        <span key={key} className="inline-flex items-center gap-1.5">
          <span className="size-2.5 rounded-[3px]" style={{ background: item.color }} />
          {item.label ?? key}
        </span>
      ))}
    </div>
  )
}

export { ChartContainer, ChartTooltip, ChartTooltipContent, ChartLegendContent, ChartStyle }
