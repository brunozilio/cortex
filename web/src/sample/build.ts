import type { ContextSplit, Run, Signal, Span, SpanKind, Usage } from '@/types'

const PRICES: Record<string, [number, number, number, number]> = {
  'sonnet-4.6': [3, 15, 0.3, 3.75],
  'opus-4.6': [15, 75, 1.5, 18.75],
  'haiku-4.5': [1, 5, 0.1, 1.25],
}

export function usage(model: string, input: number, cacheRead: number, cacheWrite: number, output: number): Usage {
  const [inPrice, outPrice, readPrice, writePrice] = PRICES[model] ?? PRICES['sonnet-4.6']
  const cost = (input * inPrice + output * outPrice + cacheRead * readPrice + cacheWrite * writePrice) / 1e6
  return { model, input, output, cache_read: cacheRead, cache_write: cacheWrite, cost_usd: cost }
}

export function context(system: number, tools: number, conversation: number): ContextSplit {
  return { system, tools, conversation }
}

type SpanInit = Partial<Span> & { id: number; parent: number; kind: SpanKind; name: string; start_ms: number; end_ms: number }

export function span(init: SpanInit): Span {
  return {
    failed: false,
    input: '',
    output: '',
    thinking: '',
    usage: null,
    context: null,
    stop: null,
    attrs: [],
    flags: [],
    ...init,
  }
}

export function tool(id: number, parent: number, toolName: string, summary: string, start: number, end: number, extra: Partial<Span> = {}): Span {
  return span({
    id,
    parent,
    kind: 'tool',
    name: `${toolName} ${summary}`,
    start_ms: start,
    end_ms: end,
    ...extra,
    attrs: [['tool', toolName], ...(extra.attrs ?? [])],
  })
}

interface RunInit {
  session: string
  index: number
  prompt: string
  started_at_ms: number
  model: string
  status: Run['status']
  spans: Span[]
  signals?: Signal[]
}

export function run(init: RunInit): Run {
  const root = init.spans.find((entry) => entry.parent === 0)
  const duration = root ? root.end_ms - root.start_ms : 0
  const calls = init.spans.filter((entry) => entry.usage).map((entry) => entry.usage as Usage)
  const input = calls.reduce((sum, call) => sum + call.input + call.cache_read + call.cache_write, 0)
  const output = calls.reduce((sum, call) => sum + call.output, 0)
  const wasted = init.spans
    .filter((entry) => entry.parent !== 0 && (entry.failed || entry.flags.includes('repeated')) && entry.kind !== 'event')
    .reduce((sum, entry) => sum + entry.end_ms - entry.start_ms, 0)
  const waste = duration ? Math.min(1, wasted / duration) : 0
  const signals = new Set<Signal>(init.signals ?? [])
  if (init.spans.some((entry) => entry.kind === 'tool' && entry.failed)) signals.add('tool_error')
  if (init.spans.some((entry) => entry.kind === 'agent')) signals.add('subagent')
  if (init.spans.some((entry) => entry.flags.includes('repeated'))) signals.add('repeated')
  if (waste > 0.2) signals.add('waste')
  return {
    session: init.session,
    index: init.index,
    prompt: init.prompt,
    started_at_ms: init.started_at_ms,
    duration_ms: duration,
    waiting_ms: init.spans.filter((entry) => entry.kind === 'wait').reduce((sum, entry) => sum + entry.end_ms - entry.start_ms, 0),
    model: init.model,
    status: init.status,
    cost_usd: calls.reduce((sum, call) => sum + call.cost_usd, 0),
    tokens: input + output,
    input,
    cache_read: calls.reduce((sum, call) => sum + call.cache_read, 0),
    waste,
    signals: [...signals],
    spans: init.spans,
  }
}
