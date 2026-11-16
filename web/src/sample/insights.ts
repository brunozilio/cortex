import type { Day, Finding, Insights, SessionRuns } from '@/types'
import { dayOf } from '@/lib/format'

function seeded(seed: number) {
  let state = seed
  return () => {
    state = (state * 16807) % 2147483647
    return state / 2147483647
  }
}

function sampleDays(now: number): Day[] {
  const random = seeded(42)
  const today = dayOf(now)
  return Array.from({ length: 30 }, (_, i) => {
    const day = today - 29 + i
    const weekend = [3, 4].includes(day % 7)
    const sonnet = weekend ? random() * 0.3 : 0.5 + random() * 1.4
    const opus = random() < 0.18 ? 0.6 + random() * 1.6 : 0
    const haiku = weekend ? 0 : 0.02 + random() * 0.08
    const models = [
      { model: 'sonnet-4.6', cost_usd: sonnet, tokens: Math.round(sonnet * 410e3), runs: Math.round(sonnet * 4 + (weekend ? 0 : 1)) },
      { model: 'opus-4.6', cost_usd: opus, tokens: Math.round(opus * 95e3), runs: Math.round(opus * 1.4) },
      { model: 'haiku-4.5', cost_usd: haiku, tokens: Math.round(haiku * 2.1e6), runs: Math.round(haiku * 30) },
    ].filter((entry) => entry.cost_usd > 0)
    return { day, models }
  }).filter((_, i) => i % 11 !== 5)
}

function sampleFindings(sessions: SessionRuns[], now: number): Finding[] {
  const rich = sessions.find((s) => s.id === '4f2a91c3')?.runs.find((r) => r.index === 3)
  const pdf = sessions.find((s) => s.id === '9b07d2e1')?.runs.find((r) => r.index === 2)
  const flaky = sessions.find((s) => s.id === '7d21aa04')?.runs.find((r) => r.index === 2)
  const findings: Finding[] = []
  if (rich) {
    findings.push({ session: rich.session, run: rich.index, span: 22, check: 'review', text: 'docs/billing.md still says "rounded half up" (line 41). Update it to say interest is rounded half-even.', at_ms: rich.started_at_ms + 93800, severe: false })
    findings.push({ session: rich.session, run: rich.index, span: 12, check: 'repeated', text: 'Bash cargo test -p billing failed twice with the same output; the loop guard refused a third try.', at_ms: rich.started_at_ms + 47400, severe: false })
  }
  const judgeOf = (run: typeof pdf, text: string) => {
    const judge = run?.spans.find((s) => s.kind === 'eval')
    if (run && judge) findings.push({ session: run.session, run: run.index, span: judge.id, check: 'judge', text, at_ms: run.started_at_ms + judge.end_ms, severe: true })
  }
  judgeOf(pdf, 'Criteria not met: the new PDF snapshot test never ran.')
  judgeOf(flaky, 'webhook_retries still fails 1 in 5 runs.')
  return findings.sort((a, b) => b.at_ms - a.at_ms).filter((f) => f.at_ms <= now + 60_000)
}

export function sampleInsights(sessions: SessionRuns[], now: number): Insights {
  const days = sampleDays(now)
  const sum = (pick: (m: Day['models'][number]) => number) => days.reduce((a, d) => a + d.models.reduce((b, m) => b + pick(m), 0), 0)
  return {
    days,
    models: [
      { model: 'sonnet-4.6', calls: 412, p50_ms: 4800, p95_ms: 11200, cache_ratio: 0.84, cost_usd: 21.4 },
      { model: 'opus-4.6', calls: 19, p50_ms: 9600, p95_ms: 21000, cache_ratio: 0.22, cost_usd: 5.1 },
      { model: 'haiku-4.5', calls: 88, p50_ms: 1100, p95_ms: 2900, cache_ratio: 0.71, cost_usd: 0.9 },
    ],
    tools: [
      { name: 'Bash', calls: 188, failed: 23, p95_ms: 14100, asked: 19 },
      { name: 'Read', calls: 341, failed: 3, p95_ms: 40, asked: 0 },
      { name: 'Edit', calls: 122, failed: 9, p95_ms: 60, asked: 0 },
      { name: 'Grep', calls: 97, failed: 0, p95_ms: 150, asked: 0 },
      { name: 'WebFetch', calls: 11, failed: 2, p95_ms: 2300, asked: 4 },
    ],
    time: [
      { kind: 'llm', ms: 1_320_000 },
      { kind: 'tool', ms: 780_000 },
      { kind: 'wait', ms: 300_000 },
      { kind: 'agent', ms: 180_000 },
      { kind: 'task', ms: 120_000 },
      { kind: 'eval', ms: 40_000 },
    ],
    checks: { verified_passed: 38, verified_failed: 4, review_findings: 6, repeated: 3, refused: 2, asked: 23 },
    totals: {
      runs: Math.round(sum((m) => m.runs)),
      cost_usd: sum((m) => m.cost_usd),
      tokens: Math.round(sum((m) => m.tokens)),
      waiting_ms: 300_000,
      p95_run_ms: 372_000,
      tool_error_rate: 0.049,
      cache_ratio: 0.81,
    },
    findings: sampleFindings(sessions, now),
  }
}
