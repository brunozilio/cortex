import type { Finding, Run, Span, Usage } from '@/types'

export interface PlacedSpan {
  span: Span
  depth: number
}

export function treeOrder(spans: Span[]): PlacedSpan[] {
  const children = new Map<number, Span[]>()
  for (const span of spans) {
    const list = children.get(span.parent) ?? []
    list.push(span)
    children.set(span.parent, list)
  }
  const out: PlacedSpan[] = []
  const seen = new Set<number>()
  const walk = (parent: number, depth: number) => {
    const list = (children.get(parent) ?? []).slice().sort((a, b) => a.start_ms - b.start_ms || a.id - b.id)
    for (const span of list) {
      if (seen.has(span.id)) continue
      seen.add(span.id)
      out.push({ span, depth })
      walk(span.id, depth + 1)
    }
  }
  walk(0, 0)
  for (const span of spans) {
    if (seen.has(span.id)) continue
    seen.add(span.id)
    out.push({ span, depth: 1 })
    walk(span.id, 2)
  }
  return out
}

export function rootSpan(run: Run): Span | undefined {
  return run.spans.find((span) => span.parent === 0) ?? run.spans[0]
}

export function timelineEnd(run: Run): number {
  const latest = run.spans.reduce((max, span) => Math.max(max, span.end_ms), 0)
  return Math.max(run.duration_ms, latest, 1)
}

export function spanById(run: Run, id: number): Span | undefined {
  return run.spans.find((span) => span.id === id)
}

export function promptTokens(usage: Usage): number {
  return usage.input + usage.cache_read + usage.cache_write
}

export function callCacheRatio(usage: Usage): number {
  const total = promptTokens(usage)
  return total ? usage.cache_read / total : 0
}

export function runCacheRatio(run: Run): number {
  return run.input ? run.cache_read / run.input : 0
}

export function modelCalls(run: Run): Span[] {
  return run.spans.filter((span) => span.kind === 'llm' && span.usage)
}

export function priciestCall(run: Run): Span | undefined {
  return run.spans
    .filter((span) => span.usage)
    .reduce<Span | undefined>((best, span) => (!best || (span.usage?.cost_usd ?? 0) > (best.usage?.cost_usd ?? 0) ? span : best), undefined)
}

export function attr(span: Span, key: string): string | undefined {
  return span.attrs.find(([name]) => name === key)?.[1]
}

export function isRepeated(span: Span): boolean {
  return span.flags.some((flag) => flag === 'repeated')
}

export function refusalReason(span: Span): string | undefined {
  const flag = span.flags.find((value) => value.startsWith('refused'))
  if (!flag) return undefined
  const reason = flag.slice('refused'.length).replace(/^:\s*/, '')
  return reason || 'refused'
}

export function findingsFor(findings: Finding[], run: Run, span?: Span): Finding[] {
  return findings.filter(
    (finding) => finding.session === run.session && finding.run === run.index && (!span || span.parent === 0 || finding.span === span.id),
  )
}
