import type { ReactNode } from 'react'
import { Activity, AlertCircle, Clock, DollarSign, TrendingDown, TrendingUp, type LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Sparkline } from '@/components/Sparkline'
import { dailySeries, weekOverWeek } from '@/lib/days'
import { ms, percent, usdShort } from '@/lib/format'
import type { Report } from '@/types'

interface KpiProps {
  label: string
  icon: LucideIcon
  value: string
  footer: ReactNode
  chart?: ReactNode
}

function Kpi({ label, icon: Icon, value, footer, chart }: KpiProps) {
  return (
    <Card className="flex flex-col">
      <div className="grid gap-1 p-5 pb-0">
        <div className="flex items-center justify-between text-sm text-muted-foreground">
          <span>{label}</span>
          <Icon className="size-4" />
        </div>
        <div className="num text-2xl font-semibold tracking-tight">{value}</div>
        <div className="flex min-h-6 items-center gap-1.5 text-xs text-muted-foreground">{footer}</div>
      </div>
      <div className="mt-auto overflow-hidden rounded-b-xl pt-3">{chart ?? <div className="h-9" />}</div>
    </Card>
  )
}

function Trend({ change, upIsGood }: { change: number | null; upIsGood: boolean }) {
  if (change === null) return <span>no data the week before</span>
  if (Math.abs(change) < 0.005) {
    return (
      <>
        <Badge variant="secondary">flat</Badge>
        <span>last 7 days vs the 7 before</span>
      </>
    )
  }
  const up = change >= 0
  const Icon = up ? TrendingUp : TrendingDown
  return (
    <>
      <Badge variant={up === upIsGood ? 'success' : 'danger'}>
        <Icon />
        {up ? '+' : '−'}
        {percent(Math.abs(change))}
      </Badge>
      <span>last 7 days vs the 7 before</span>
    </>
  )
}

export function KpiCards({ report }: { report: Report }) {
  const { totals, days } = report.insights
  const cost = dailySeries(days, 'cost', report.generated_at_ms)
  const runs = dailySeries(days, 'runs', report.generated_at_ms)
  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <Kpi
        label="Spend"
        icon={DollarSign}
        value={usdShort(totals.cost_usd)}
        footer={<Trend change={weekOverWeek(cost)} upIsGood={false} />}
        chart={<Sparkline values={cost.slice(-14).map((row) => row.total)} color="hsl(var(--k-llm))" />}
      />
      <Kpi
        label="Runs"
        icon={Activity}
        value={String(totals.runs)}
        footer={<Trend change={weekOverWeek(runs)} upIsGood />}
        chart={<Sparkline values={runs.slice(-14).map((row) => row.total)} color="hsl(var(--k-agent))" />}
      />
      <Kpi
        label="p95 run time"
        icon={Clock}
        value={ms(totals.p95_run_ms)}
        footer={<span>{ms(totals.waiting_ms)} of all run time was waiting on you</span>}
      />
      <Kpi
        label="Tool error rate"
        icon={AlertCircle}
        value={(totals.tool_error_rate * 100).toFixed(1) + '%'}
        footer={<span>of all tool calls failed</span>}
        chart={
          <div className="px-5 pb-5">
            <Progress
              value={Math.min(100, totals.tool_error_rate * 400)}
              indicatorClassName={totals.tool_error_rate > 0.1 ? 'bg-danger' : totals.tool_error_rate > 0.05 ? 'bg-warning' : 'bg-success'}
            />
          </div>
        }
      />
    </div>
  )
}
