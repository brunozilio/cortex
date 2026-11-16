import { GitBranch, Sparkles } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { SignalBadges, StatusIcon } from '@/components/RunBadges'
import { ms, percent, shortId, tokens, usd, when } from '@/lib/format'
import { modelCalls, priciestCall, runCacheRatio, timelineEnd } from '@/lib/spans'
import { cn } from '@/lib/utils'
import type { Run } from '@/types'

function Stat({ label, value, sub, className }: { label: string; value: string; sub?: string; className?: string }) {
  return (
    <div className="grid content-start gap-0.5 border-t px-5 py-3 sm:border-l">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className={cn('num text-lg font-semibold', className)}>{value}</span>
      {sub ? <span className="text-xs text-muted-foreground">{sub}</span> : null}
    </div>
  )
}

function costNote(run: Run): string {
  const top = priciestCall(run)
  if (!top?.usage || !run.cost_usd) return ''
  const share = top.usage.cost_usd / run.cost_usd
  return share >= 0.4 ? `${percent(share)} in one ${top.kind === 'llm' ? 'model' : top.kind} call` : ''
}

export function RunHeader({ run, current, nowMs }: { run: Run; current: string; nowMs: number }) {
  const tools = run.spans.filter((span) => span.kind === 'tool')
  const failedTools = tools.filter((span) => span.failed).length
  return (
    <>
      <div className="flex flex-wrap items-start gap-3 p-5">
        <StatusIcon status={run.status} chip />
        <div className="grid min-w-0 flex-1 gap-1.5">
          <h2 className="text-balance font-semibold leading-snug">{run.prompt}</h2>
          <div className="flex flex-wrap gap-1.5">
            <Badge variant="secondary">run {run.index}</Badge>
            <Badge>
              <GitBranch />
              session {shortId(run.session)}
              {run.session === current ? ' (current)' : ''}
            </Badge>
            <Badge>
              <Sparkles />
              {run.model}
            </Badge>
            <SignalBadges signals={run.signals} />
          </div>
        </div>
        <span className="text-xs text-muted-foreground">{when(run.started_at_ms, nowMs)}</span>
      </div>
      <div className="grid grid-cols-2 border-b sm:grid-cols-3 lg:grid-cols-6 [&>*:first-child]:sm:border-l-0">
        <Stat label="Duration" value={ms(timelineEnd(run))} sub={run.waiting_ms ? `${ms(run.waiting_ms)} waiting on you` : 'agent time only'} />
        <Stat label="Cost" value={usd(run.cost_usd)} sub={costNote(run)} />
        <Stat label="Tokens" value={tokens(run.tokens)} sub={`${percent(runCacheRatio(run))} of input from cache`} />
        <Stat label="Model calls" value={String(modelCalls(run).length)} />
        <Stat label="Tool calls" value={String(tools.length)} sub={failedTools ? `${failedTools} failed` : 'none failed'} className={failedTools ? 'text-danger' : ''} />
        <Stat label="Wasted" value={percent(run.waste)} sub="failed and repeated work" className={run.waste > 0.2 ? 'text-warning' : ''} />
      </div>
    </>
  )
}
