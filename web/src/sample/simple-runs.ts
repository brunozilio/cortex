import type { Run, Signal, Span } from '@/types'
import { context, run, span, tool, usage } from './build'

export interface SimpleRunSpec {
  session: string
  index: number
  hoursAgo: number
  prompt: string
  model: string
  status: Run['status']
  durationMs: number
  failTool?: boolean
  subagent?: boolean
  evalFails?: boolean
  compacted?: boolean
  fellBack?: boolean
  signals?: Signal[]
}

const TOOLS: [string, string][] = [['Read', 'src/lib.rs'], ['Grep', 'TODO'], ['Edit', 'src/handler.rs'], ['Bash', 'cargo test']]

export function simpleRun(spec: SimpleRunSpec, now: number): Run {
  const total = spec.durationMs
  const step = total / 7
  const spans: Span[] = [span({ id: 1, parent: 0, kind: 'run', name: 'run', start_ms: 0, end_ms: total, input: spec.prompt, output: spec.status === 'ok' ? 'Done.' : 'Stopped before finishing.', attrs: [['session', spec.session]] })]
  let id = 2
  let t = 0
  TOOLS.forEach(([name, summary], i) => {
    const conversation = 3000 + i * 2600
    spans.push(span({
      id: id++, parent: 1, kind: 'llm', name: spec.model, start_ms: t + 100, end_ms: t + step * 0.8, stop: 'tool_use',
      usage: usage(spec.model, 900 + i * 300, i ? 9000 + i * 2400 : 0, i ? 0 : 9000, 380), context: context(4100, 8000, conversation),
      input: i ? `[tool result] ${TOOLS[i - 1][0]}` : spec.prompt, output: `→ ${name}(${summary})`,
    }))
    const failed = Boolean(spec.failTool) && i === 3
    spans.push(tool(id++, 1, name, summary, t + step * 0.8, t + step * (i === 3 ? 1.6 : 1), {
      failed,
      input: name === 'Bash' ? 'cargo test --workspace' : summary,
      output: failed ? 'error[E0425]: cannot find value `pool` in this scope\n --> src/handler.rs:42:9' : 'ok',
    }))
    t += step * (i === 3 ? 1.6 : 1)
  })
  if (spec.compacted) spans.push(span({ id: id++, parent: 1, kind: 'event', name: 'compacted context', start_ms: t, end_ms: t, output: 'Conversation summarised to 12k tokens.' }))
  if (spec.fellBack) spans.push(span({ id: id++, parent: 1, kind: 'event', name: `fell back to haiku-4.5`, start_ms: t, end_ms: t, output: 'Primary model overloaded.' }))
  if (spec.subagent) {
    const agent = id++
    spans.push(span({ id: agent, parent: 1, kind: 'agent', name: 'subagent · explore', start_ms: t, end_ms: t + step * 2, input: 'Map the modules that touch invoices.', output: 'Found 6 modules.', attrs: [['subagent', 'explore']] }))
    spans.push(span({ id: id++, parent: agent, kind: 'llm', name: 'haiku-4.5', start_ms: t + 50, end_ms: t + step * 1.9, stop: 'end_turn', usage: usage('haiku-4.5', 18000, 12000, 0, 900), context: context(2000, 3000, 25000), input: 'Map the modules that touch invoices.', output: 'Found 6 modules.' }))
    t += step * 2
  }
  spans.push(span({
    id: id++, parent: 1, kind: 'eval', name: 'verification judge', start_ms: Math.max(t, total - 1900), end_ms: total, failed: Boolean(spec.evalFails),
    usage: usage(spec.model, 8000, 0, 0, 200), input: 'criteria + diff',
    output: spec.evalFails ? 'Criteria not met: the new test never ran.' : 'Criteria met.',
  }))
  const signals: Signal[] = [...(spec.signals ?? [])]
  if (spec.evalFails) signals.push('eval_failed')
  if (spec.compacted) signals.push('compacted')
  if (spec.fellBack) signals.push('fell_back')
  return run({
    session: spec.session,
    index: spec.index,
    prompt: spec.prompt,
    started_at_ms: now - spec.hoursAgo * 3_600_000,
    model: spec.model,
    status: spec.status,
    signals,
    spans,
  })
}
