import type { ReactNode } from 'react'
import { Activity, AlertCircle, Clock, DollarSign, Minus, TrendingDown, TrendingUp, type LucideIcon } from 'lucide-react'
import { Badge, type BadgeVariant } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Sparkline } from '@/components/Sparkline'
import { dailySeries, weekOverWeek } from '@/lib/days'
import { ms, percent, usdShort } from '@/lib/format'
import type { Strings } from '@/lib/i18n'
import { useStrings } from '@/shell/strings'
import type { Report } from '@/types'

type Direction = 'up-is-good' | 'down-is-good' | 'neutral'

const ERROR_RATE_DANGER = 0.05

interface KpiProps {
  label: string
  icon: LucideIcon
  value: string
  chip?: ReactNode
  caption: string
  visual: ReactNode
}

function Kpi({ label, icon: Icon, value, chip, caption, visual }: KpiProps) {
  return (
    <Card className="flex flex-col gap-3 p-5">
      <div className="flex items-center justify-between text-sm text-muted-foreground">
        <span>{label}</span>
        <Icon className="size-4" />
      </div>
      <div className="grid gap-1.5">
        <div className="num text-2xl font-semibold leading-none tracking-tight">{value}</div>
        <div className="flex min-h-5 items-center gap-1.5 text-xs text-muted-foreground">
          {chip}
          <span className="truncate">{caption}</span>
        </div>
      </div>
      <div className="mt-auto flex h-8 items-end">{visual}</div>
    </Card>
  )
}

function trendVariant(up: boolean, direction: Direction): BadgeVariant {
  if (direction === 'neutral') return 'secondary'
  return up === (direction === 'up-is-good') ? 'success' : 'warning'
}

function TrendChip({ change, direction }: { change: number | null; direction: Direction }) {
  const strings = useStrings()
  if (change === null) return null
  if (Math.abs(change) < 0.005) {
    return (
      <Badge variant="secondary">
        <Minus />
        {strings.kpi.flat}
      </Badge>
    )
  }
  const up = change >= 0
  const Icon = up ? TrendingUp : TrendingDown
  return (
    <Badge variant={trendVariant(up, direction)}>
      <Icon />
      {up ? '+' : '−'}
      {percent(Math.abs(change))}
    </Badge>
  )
}

function Meter({ ratio, className }: { ratio: number; className: string }) {
  return <Progress value={ratio * 100} className="h-2" indicatorClassName={className} />
}

function trendCaption(change: number | null, strings: Strings): string {
  return change === null ? strings.kpi.noPrior : strings.kpi.weekOverWeek
}

export function KpiCards({ report }: { report: Report }) {
  const strings = useStrings()
  const text = strings.kpi
  const { totals, days, time } = report.insights
  const cost = dailySeries(days, 'cost', report.generated_at_ms)
  const runs = dailySeries(days, 'runs', report.generated_at_ms)
  const costChange = weekOverWeek(cost)
  const runsChange = weekOverWeek(runs)
  const runTime = time.reduce((sum, entry) => sum + entry.ms, 0)
  const waitingShare = runTime ? totals.waiting_ms / runTime : 0
  const errorRate = totals.tool_error_rate
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Kpi
        label={text.spend}
        icon={DollarSign}
        value={usdShort(totals.cost_usd)}
        chip={<TrendChip change={costChange} direction="down-is-good" />}
        caption={trendCaption(costChange, strings)}
        visual={<Sparkline values={cost.slice(-14).map((row) => row.total)} color="hsl(var(--chart-1))" />}
      />
      <Kpi
        label={text.runs}
        icon={Activity}
        value={String(totals.runs)}
        chip={<TrendChip change={runsChange} direction="neutral" />}
        caption={trendCaption(runsChange, strings)}
        visual={<Sparkline values={runs.slice(-14).map((row) => row.total)} color="hsl(var(--chart-1))" />}
      />
      <Kpi
        label={text.p95}
        icon={Clock}
        value={ms(totals.p95_run_ms)}
        chip={<Badge variant="secondary">{percent(waitingShare)}</Badge>}
        caption={text.waiting(ms(totals.waiting_ms))}
        visual={<Meter ratio={waitingShare} className="bg-muted-foreground" />}
      />
      <Kpi
        label={text.errorRate}
        icon={AlertCircle}
        value={(errorRate * 100).toFixed(1) + '%'}
        chip={errorRate > ERROR_RATE_DANGER ? <Badge variant="danger">{text.high}</Badge> : null}
        caption={text.failedCalls}
        visual={<Meter ratio={errorRate} className={errorRate > ERROR_RATE_DANGER ? 'bg-danger' : 'bg-muted-foreground'} />}
      />
    </div>
  )
}
