import { Ban, Hand, Layers, MessageSquare, ShieldCheck, type LucideIcon } from 'lucide-react'
import { Badge, type BadgeVariant } from '@/components/ui/badge'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { KindChip } from '@/components/KindChip'
import { Ring } from '@/components/Ring'
import { ms, percent } from '@/lib/format'
import { KINDS, MODEL_COLORS } from '@/lib/kinds'
import { modelsByCost } from '@/lib/days'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Insights } from '@/types'

export function TimeCard({ insights }: { insights: Insights }) {
  const strings = useStrings()
  const labels = strings.time.kinds
  const rows = insights.time.filter((entry) => labels[entry.kind] && entry.ms > 0).sort((a, b) => b.ms - a.ms)
  const total = rows.reduce((sum, entry) => sum + entry.ms, 0) || 1
  return (
    <Card>
      <CardHeader>
        <CardTitle>{strings.time.title}</CardTitle>
        <CardDescription>{strings.time.description}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4">
        <div className="flex h-3 overflow-hidden rounded-full bg-muted">
          {rows.map((entry) => (
            <div key={entry.kind} className={cn('h-full', KINDS[entry.kind].bar)} style={{ width: `${(entry.ms / total) * 100}%` }} />
          ))}
        </div>
        <div className="grid gap-2.5 text-sm">
          {rows.map((entry) => (
            <div key={entry.kind} className="flex items-center gap-2.5">
              <KindChip kind={entry.kind} />
              <span>{labels[entry.kind]}</span>
              <span className="num ml-auto text-muted-foreground">{ms(entry.ms)}</span>
              <span className="num w-10 text-right font-medium">{percent(entry.ms / total)}</span>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}

export function CacheCard({ insights }: { insights: Insights }) {
  const strings = useStrings()
  const order = modelsByCost(insights.days)
  const colorOf = (model: string) => {
    const i = order.indexOf(model)
    return MODEL_COLORS[(i < 0 ? order.length : i) % MODEL_COLORS.length]
  }
  return (
    <Card>
      <CardHeader>
        <CardTitle>{strings.cache.title}</CardTitle>
        <CardDescription>{strings.cache.description}</CardDescription>
      </CardHeader>
      <CardContent className="flex items-center gap-5">
        <Ring ratio={insights.totals.cache_ratio} color="hsl(var(--k-llm))" size={112} />
        <div className="grid flex-1 gap-2.5 text-sm">
          {insights.models.map((model) => (
            <div key={model.model} className="grid gap-1">
              <div className="flex items-center gap-2">
                <span className="size-2 rounded-full" style={{ background: colorOf(model.model) }} />
                <span className="truncate font-mono text-[13px]">{model.model}</span>
                <span className="num ml-auto">{percent(model.cache_ratio)}</span>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-muted">
                <div className="h-full" style={{ width: `${model.cache_ratio * 100}%`, background: colorOf(model.model) }} />
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}

interface CheckRow {
  icon: LucideIcon
  label: string
  value: string
  variant: BadgeVariant
}

export function ChecksCard({ insights }: { insights: Insights }) {
  const strings = useStrings()
  const text = strings.checks
  const { checks } = insights
  const rows: CheckRow[] = [
    { icon: ShieldCheck, label: text.judge, value: text.judged(checks.verified_passed, checks.verified_failed), variant: checks.verified_failed ? 'danger' : 'success' },
    { icon: MessageSquare, label: text.review, value: text.findings(checks.review_findings), variant: checks.review_findings ? 'warning' : 'secondary' },
    { icon: Layers, label: text.loops, value: String(checks.repeated), variant: 'secondary' },
    { icon: Ban, label: text.refused, value: String(checks.refused), variant: checks.refused ? 'warning' : 'secondary' },
    { icon: Hand, label: text.asked, value: String(checks.asked), variant: 'secondary' },
  ]
  return (
    <Card>
      <CardHeader>
        <CardTitle>{text.title}</CardTitle>
        <CardDescription>{text.description}</CardDescription>
      </CardHeader>
      <CardContent className="grid gap-2 text-sm">
        {rows.map(({ icon: Icon, label, value, variant }) => (
          <div key={label} className="flex items-center gap-2.5 rounded-lg border px-3 py-2">
            <Icon className="size-4 text-muted-foreground" />
            <span>{label}</span>
            <Badge variant={variant} className="ml-auto">
              {value}
            </Badge>
          </div>
        ))}
      </CardContent>
    </Card>
  )
}
