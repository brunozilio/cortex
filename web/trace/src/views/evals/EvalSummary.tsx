import type { ReactNode } from 'react'
import { Ban, Hand, Layers, MessageSquare, ShieldCheck, type LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card } from '@/components/ui/card'
import { Ring } from '@/components/Ring'
import { percent } from '@/lib/format'
import type { Checks } from '@/types'

interface SummaryCardProps {
  icon: LucideIcon
  label: string
  value: string
  caption: string
  aside?: ReactNode
  detail?: ReactNode
}

function SummaryCard({ icon: Icon, label, value, caption, aside, detail }: SummaryCardProps) {
  return (
    <Card className="flex flex-col gap-3 p-5">
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <Icon className="size-4 shrink-0" />
        <span className="truncate">{label}</span>
      </div>
      <div className="flex items-center justify-between gap-3">
        <div className="grid min-w-0 gap-1.5">
          <span className="num text-2xl font-semibold leading-none tracking-tight">{value}</span>
          <span className="text-xs text-muted-foreground">{caption}</span>
        </div>
        {aside}
      </div>
      <div className="mt-auto flex min-h-5 flex-wrap gap-1.5">{detail}</div>
    </Card>
  )
}

export function EvalSummary({ checks, severeReviews }: { checks: Checks; severeReviews: number }) {
  const judged = checks.verified_passed + checks.verified_failed
  const passRate = judged ? checks.verified_passed / judged : 0
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5">
      <SummaryCard
        icon={ShieldCheck}
        label="Verification judge"
        value={judged ? percent(passRate) : '–'}
        caption="met the run's criteria"
        aside={
          judged ? (
            <Ring ratio={passRate} color="hsl(var(--success))" trackColor="hsl(var(--danger))" size={44} stroke={6} label="" />
          ) : null
        }
        detail={
          <>
            <Badge variant="success">{checks.verified_passed} passed</Badge>
            <Badge variant={checks.verified_failed ? 'danger' : 'secondary'}>{checks.verified_failed} failed</Badge>
          </>
        }
      />
      <SummaryCard
        icon={MessageSquare}
        label="Auto-review"
        value={String(checks.review_findings)}
        caption="findings where the diff contradicts the docs"
        detail={severeReviews ? <Badge variant="danger">{severeReviews} severe</Badge> : null}
      />
      <SummaryCard icon={Layers} label="Loops stopped" value={String(checks.repeated)} caption="same failing call repeated" />
      <SummaryCard icon={Ban} label="Calls refused" value={String(checks.refused)} caption="calls the agent was not allowed to make" />
      <SummaryCard icon={Hand} label="Permission asks" value={String(checks.asked)} caption="times the agent waited for you" />
    </div>
  )
}
