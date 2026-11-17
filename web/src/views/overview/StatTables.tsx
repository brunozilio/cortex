import { AlertCircle } from 'lucide-react'
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { Callout } from '@/components/Callout'
import { modelsByCost } from '@/lib/days'
import { ms, percent, usd } from '@/lib/format'
import { MODEL_COLORS } from '@/lib/kinds'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Insights, ToolStat } from '@/types'

function Meter({ ratio, fill, className, color }: { ratio: number; fill?: number; className?: string; color?: string }) {
  return (
    <div className="flex items-center justify-end gap-2">
      <div className="h-1.5 w-14 overflow-hidden rounded-full bg-muted">
        <div className={cn('h-full', className)} style={{ width: `${Math.min(100, (fill ?? ratio) * 100)}%`, background: color }} />
      </div>
      <span className="num w-9 text-right">{percent(ratio)}</span>
    </div>
  )
}

export function ModelTable({ insights }: { insights: Insights }) {
  const text = useStrings().models
  const order = modelsByCost(insights.days)
  return (
    <Card>
      <CardHeader>
        <CardTitle>{text.title}</CardTitle>
        <CardDescription>{text.description}</CardDescription>
      </CardHeader>
      <Table>
        <TableHeader>
          <TableRow className="border-t">
            <TableHead>{text.model}</TableHead>
            <TableHead className="text-right">{text.calls}</TableHead>
            <TableHead className="text-right">p50</TableHead>
            <TableHead className="text-right">p95</TableHead>
            <TableHead className="text-right">{text.cache}</TableHead>
            <TableHead className="text-right">{text.cost}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {insights.models.map((model) => {
            const i = order.indexOf(model.model)
            const color = MODEL_COLORS[(i < 0 ? order.length : i) % MODEL_COLORS.length]
            return (
              <TableRow key={model.model}>
                <TableCell>
                  <span className="inline-flex items-center gap-2 font-mono text-[13px]">
                    <span className="size-2 rounded-full" style={{ background: color }} />
                    {model.model}
                  </span>
                </TableCell>
                <TableCell className="num text-right">{model.calls}</TableCell>
                <TableCell className="num text-right">{ms(model.p50_ms)}</TableCell>
                <TableCell className="num text-right">{ms(model.p95_ms)}</TableCell>
                <TableCell>
                  <Meter ratio={model.cache_ratio} color={color} />
                </TableCell>
                <TableCell className="num text-right font-medium">{usd(model.cost_usd)}</TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </Card>
  )
}

function errorRate(tool: ToolStat): number {
  return tool.calls ? tool.failed / tool.calls : 0
}

function rateColor(rate: number): string {
  if (rate > 0.1) return 'bg-danger'
  if (rate > 0.05) return 'bg-warning'
  return 'bg-success'
}

export function ToolTable({ insights }: { insights: Insights }) {
  const text = useStrings().tools
  const worst = insights.tools.filter((tool) => tool.calls >= 5).sort((a, b) => errorRate(b) - errorRate(a))[0]
  return (
    <Card>
      <CardHeader>
        <CardTitle>{text.title}</CardTitle>
        <CardDescription>{text.description}</CardDescription>
      </CardHeader>
      <Table>
        <TableHeader>
          <TableRow className="border-t">
            <TableHead>{text.tool}</TableHead>
            <TableHead className="text-right">{text.calls}</TableHead>
            <TableHead className="text-right">{text.errorRate}</TableHead>
            <TableHead className="text-right">p95</TableHead>
            <TableHead className="text-right">{text.asked}</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {insights.tools.map((tool) => {
            const rate = errorRate(tool)
            return (
              <TableRow key={tool.name}>
                <TableCell className="font-mono text-[13px]">{tool.name}</TableCell>
                <TableCell className="num text-right">{tool.calls}</TableCell>
                <TableCell>
                  <Meter ratio={rate} fill={Math.max(rate * 4, rate ? 0.03 : 0)} className={rateColor(rate)} />
                </TableCell>
                <TableCell className="num text-right">{ms(tool.p95_ms)}</TableCell>
                <TableCell className="num text-right">{tool.asked}</TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
      {worst && errorRate(worst) > 0.1 ? (
        <Callout tone="warning" icon={AlertCircle} title={text.worst(worst.name, worst.failed, worst.calls)} className="m-4 mt-3">
          {text.worstDetail(percent(errorRate(worst)))}
        </Callout>
      ) : null}
    </Card>
  )
}
