import { readFileSync } from 'node:fs'
import path from 'node:path'
import type { Insights, Report, Run, SessionRuns, Span } from '../src/types'

export function fixture(name: string): unknown {
  return JSON.parse(readFileSync(path.join(import.meta.dir, 'fixtures', name), 'utf8'))
}

export const INSIGHTS: Insights = {
  days: [],
  models: [],
  tools: [],
  time: [],
  checks: { verified_passed: 0, verified_failed: 0, review_findings: 0, repeated: 0, refused: 0, asked: 0 },
  totals: { runs: 0, cost_usd: 0, tokens: 0, waiting_ms: 0, p95_run_ms: 0, tool_error_rate: 0, cache_ratio: 0 },
  findings: [],
}

export function span(fields: Partial<Span> & { id: number }): Span {
  return {
    parent: fields.id === 1 ? 0 : 1,
    kind: 'tool',
    name: `span ${fields.id}`,
    start_ms: 0,
    end_ms: 0,
    failed: false,
    input: '',
    output: '',
    thinking: '',
    usage: null,
    context: null,
    stop: null,
    attrs: [],
    flags: [],
    ...fields,
  }
}

export function run(session: string, index: number, fields: Partial<Run> = {}): Run {
  return {
    session,
    index,
    prompt: `prompt ${index}`,
    started_at_ms: 1_000 * (index + 1),
    duration_ms: 100,
    waiting_ms: 0,
    model: 'm',
    status: 'ok',
    cost_usd: 0,
    tokens: 0,
    input: 0,
    cache_read: 0,
    waste: 0,
    signals: [],
    spans: [span({ id: 1, kind: 'run', name: 'run' })],
    ...fields,
  }
}

export function session(id: string, runs: Run[], insights?: Insights): SessionRuns {
  return insights ? { id, runs, insights } : { id, runs }
}

export function report(sessions: SessionRuns[], current = sessions[0]?.id ?? ''): Report {
  return { project: '/work/app', current, generated_at_ms: 10_000, sessions, insights: INSIGHTS }
}

export function answers(routes: Record<string, { status?: number; body: unknown } | Error>): { fetcher: typeof fetch; asked: string[] } {
  const asked: string[] = []
  const fetcher = (async (input: RequestInfo | URL) => {
    const url = String(input)
    asked.push(url)
    const answer = routes[url]
    if (answer instanceof Error) throw answer
    if (!answer) return new Response('{}', { status: 404 })
    const body = typeof answer.body === 'string' ? answer.body : JSON.stringify(answer.body)
    return new Response(body, { status: answer.status ?? 200 })
  }) as unknown as typeof fetch
  return { fetcher, asked }
}
