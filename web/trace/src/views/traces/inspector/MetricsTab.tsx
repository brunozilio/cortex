import { AlertCircle } from 'lucide-react'
import { Callout } from '@/components/Callout'
import { Ring } from '@/components/Ring'
import { ms, percent, tokens, usd } from '@/lib/format'
import { callCacheRatio, priciestCall, promptTokens } from '@/lib/spans'
import type { ContextSplit, Run, Span } from '@/types'

function Tile({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="num text-base font-semibold">{value}</div>
    </div>
  )
}

const PARTS: { key: keyof ContextSplit; label: string; color: string }[] = [
  { key: 'system', label: 'System prompt', color: 'bg-k-agent' },
  { key: 'tools', label: 'Tool definitions', color: 'bg-k-tool' },
  { key: 'conversation', label: 'Conversation', color: 'bg-k-llm' },
]

function ContextBlock({ split }: { split: ContextSplit }) {
  const total = PARTS.reduce((sum, part) => sum + split[part.key], 0) || 1
  return (
    <div className="grid gap-3">
      <div className="flex items-baseline justify-between">
        <span className="text-sm font-medium">Context window</span>
        <span className="font-mono text-xs text-muted-foreground">≈ {tokens(total)} tokens</span>
      </div>
      <div className="flex h-2.5 gap-0.5 overflow-hidden rounded-full bg-muted">
        {PARTS.map((part) => (
          <div key={part.key} className={part.color} style={{ width: `${(split[part.key] / total) * 100}%` }} />
        ))}
      </div>
      <div className="grid gap-1.5 text-sm">
        {PARTS.map((part) => (
          <div key={part.key} className="flex items-center gap-2">
            <span className={`size-2.5 rounded-[3px] ${part.color}`} />
            <span>{part.label}</span>
            <span className="ml-auto font-mono text-xs text-muted-foreground">{percent(split[part.key] / total)}</span>
            <span className="num w-14 text-right">≈ {tokens(split[part.key])}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

export function MetricsTab({ run, span }: { run: Run; span: Span }) {
  const usage = span.usage
  const top = priciestCall(run)
  const share = usage && run.cost_usd ? usage.cost_usd / run.cost_usd : 0
  const others = run.spans.filter((entry) => entry.usage && entry.id !== span.id).length
  return (
    <>
      <div className="grid grid-cols-2 gap-2">
        <Tile label="Duration" value={span.kind === 'event' ? '–' : ms(span.end_ms - span.start_ms)} />
        <Tile label="Starts at" value={ms(span.start_ms)} />
        {usage ? (
          <>
            <Tile label="Cost" value={usd(usage.cost_usd)} />
            <Tile label="Output" value={tokens(usage.output)} />
            <Tile label="Prompt" value={tokens(promptTokens(usage))} />
            <Tile label="Uncached input" value={tokens(usage.input)} />
            <Tile label="Cache read" value={tokens(usage.cache_read)} />
            <Tile label="Cache write" value={tokens(usage.cache_write)} />
          </>
        ) : null}
        {span.stop ? <Tile label="Stop reason" value={span.stop} /> : null}
      </div>
      {usage ? (
        <div className="flex items-center gap-4 rounded-lg border p-3">
          <Ring ratio={callCacheRatio(usage)} color="hsl(var(--k-llm))" size={64} />
          <div className="text-sm">
            <div className="font-medium">{tokens(usage.cache_read)} prompt tokens from cache</div>
            <div className="text-muted-foreground">
              {usage.cache_read ? 'Cache reads are billed at a fraction of the input price.' : 'Nothing was cached: this call paid full price for every prompt token.'}
            </div>
          </div>
        </div>
      ) : null}
      {span.context ? <ContextBlock split={span.context} /> : null}
      {usage && top?.id === span.id && share >= 0.4 && others > 0 ? (
        <Callout tone="warning" icon={AlertCircle} title="Most expensive call of the run">
          It cost {percent(share)} of the run, more than the other {others} priced calls together{usage.cache_read ? '' : ', with nothing served from cache'}.
        </Callout>
      ) : null}
    </>
  )
}
