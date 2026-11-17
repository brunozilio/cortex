import { AlertCircle } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { KindChip, KindSwatch } from '@/components/KindChip'
import { eventTitle } from '@/lib/events'
import { ms, tokens, usd } from '@/lib/format'
import { KINDS, LEGEND_KINDS } from '@/lib/kinds'
import { isRepeated, promptTokens, refusalReason, timelineEnd, treeOrder } from '@/lib/spans'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Run, Span } from '@/types'

const GRID = [0, 0.25, 0.5, 0.75, 1]
const COLUMNS = 'grid grid-cols-[250px_1fr_72px]'

export function Legend() {
  const strings = useStrings()
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-b px-5 py-3 text-xs text-muted-foreground">
      {LEGEND_KINDS.map((kind) => (
        <span key={kind} className="inline-flex items-center gap-1.5">
          {kind === 'event' ? <span className="size-2 rotate-45 bg-k-event" /> : <KindSwatch kind={kind} />}
          {strings.kinds[kind]}
        </span>
      ))}
      <span className="inline-flex items-center gap-1.5">
        <span className="ring-err size-2.5 rounded-[3px] bg-muted" />
        {strings.waterfall.failed}
      </span>
    </div>
  )
}

function SpanTip({ span }: { span: Span }) {
  const strings = useStrings()
  const text = strings.waterfall
  return (
    <>
      <div className="flex items-center gap-2 font-medium">
        <KindSwatch kind={span.kind} />
        {eventTitle(span, strings)}
      </div>
      <div className="mt-1.5 grid grid-cols-[auto_auto] gap-x-4 gap-y-0.5">
        {span.kind === 'event' ? null : (
          <>
            <span className="text-muted-foreground">{text.duration}</span>
            <span className="text-right font-mono">{ms(span.end_ms - span.start_ms)}</span>
          </>
        )}
        <span className="text-muted-foreground">{text.starts}</span>
        <span className="text-right font-mono">{ms(span.start_ms)}</span>
        {span.usage ? (
          <>
            <span className="text-muted-foreground">{text.tokens}</span>
            <span className="text-right font-mono">
              {tokens(promptTokens(span.usage))} → {tokens(span.usage.output)}
            </span>
            <span className="text-muted-foreground">{text.cost}</span>
            <span className="text-right font-mono">{usd(span.usage.cost_usd)}</span>
          </>
        ) : null}
        {span.failed ? <span className="col-span-2 text-danger">{text.failed}</span> : null}
      </div>
    </>
  )
}

function Bar({ span, total, running }: { span: Span; total: number; running: boolean }) {
  const left = (span.start_ms / total) * 100
  if (span.kind === 'event') {
    return (
      <div
        className={cn('absolute top-1/2 size-2.5 -translate-x-1/2 -translate-y-1/2 rotate-45 rounded-[2px]', span.failed ? 'bg-danger' : 'bg-k-event')}
        style={{ left: `${left}%` }}
      />
    )
  }
  const width = Math.max(((span.end_ms - span.start_ms) / total) * 100, 0.35)
  const cost = span.usage?.cost_usd ?? 0
  return (
    <>
      <div className={cn('absolute top-1/2 h-4 -translate-y-1/2 rounded opacity-90', KINDS[span.kind].bar, span.failed && 'ring-err', running && 'animate-pulse')} style={{ left: `${left}%`, width: `${width}%` }} />
      {cost > 0.05 ? (
        <span className={cn('absolute top-1/2 ml-1.5 -translate-y-1/2 font-mono text-[11px] font-medium', KINDS[span.kind].text)} style={{ left: `${left + width}%` }}>
          {usd(cost)}
        </span>
      ) : null}
    </>
  )
}

interface WaterfallProps {
  run: Run
  selected: number
  onSelect: (id: number) => void
}

export function Waterfall({ run, selected, onSelect }: WaterfallProps) {
  const strings = useStrings()
  const text = strings.waterfall
  const total = timelineEnd(run)
  return (
    <div className="thin-scroll overflow-x-auto">
      <div className="min-w-[620px] pb-2">
        <div className={cn(COLUMNS, 'h-8 items-center border-b font-mono text-[11px] text-muted-foreground')}>
          <span className="pl-5">{text.span}</span>
          <div className="relative h-full">
            {GRID.map((f) => (
              <span key={f} className={cn('absolute top-2 whitespace-nowrap',f === 1 ? '-translate-x-full' : f > 0 ? '-translate-x-1/2' : '')} style={{ left: `${f * 100}%` }}>
                {ms(f * total)}
              </span>
            ))}
          </div>
          <span className="pr-5 text-right">{text.time}</span>
        </div>
        <div className="relative">
          <div className="pointer-events-none absolute inset-y-0 left-[250px] right-[72px]">
            {GRID.map((f) => (
              <div key={f} className="absolute inset-y-0 border-l border-dashed" style={{ left: `${f * 100}%` }} />
            ))}
          </div>
          {treeOrder(run.spans).map(({ span, depth }) => {
            const active = span.id === selected
            const refused = refusalReason(span)
            return (
              <Tooltip key={span.id} delayDuration={250}>
                <TooltipTrigger asChild>
                  <div
                    role="button"
                    tabIndex={0}
                    aria-pressed={active}
                    onClick={() => onSelect(span.id)}
                    onKeyDown={(event) => event.key === 'Enter' && onSelect(span.id)}
                    className={cn(COLUMNS, 'relative h-9 cursor-pointer items-center text-[13px] hover:bg-muted/50', active && 'bg-muted')}
                  >
                    {active ? <span className="absolute inset-y-1 left-0 w-0.5 rounded-full bg-foreground" /> : null}
                    <div className="flex min-w-0 items-center gap-2 pr-3" style={{ paddingLeft: 16 + depth * 18 }}>
                      {depth ? <span className="-ml-1 h-5 w-2.5 self-start rounded-bl-md border-b border-l" /> : null}
                      <KindChip kind={span.kind} />
                      <span className={cn('truncate', span.kind === 'llm' && 'font-mono text-[12.5px]')}>{eventTitle(span, strings)}</span>
                      {isRepeated(span) ? <Badge variant="danger">{text.repeated}</Badge> : null}
                      {refused && !isRepeated(span) ? <Badge variant="danger">{text.refused}</Badge> : null}
                      {span.failed && !isRepeated(span) && !refused ? <AlertCircle className="size-3.5 shrink-0 text-danger" /> : null}
                    </div>
                    <div className="relative h-full">
                      <Bar span={span} total={total} running={run.status === 'running' && span.parent === 0} />
                    </div>
                    <span className="whitespace-nowrap pr-4 text-right font-mono text-xs text-muted-foreground">
                      {span.kind === 'event' ? '·' : ms(span.end_ms - span.start_ms)}
                    </span>
                  </div>
                </TooltipTrigger>
                <TooltipContent side="top" align="start">
                  <SpanTip span={span} />
                </TooltipContent>
              </Tooltip>
            )
          })}
        </div>
      </div>
    </div>
  )
}
