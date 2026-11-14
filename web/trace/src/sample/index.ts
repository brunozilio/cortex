import type { Report, SessionRuns } from '@/types'
import { CURRENT_SESSION, richRun } from './rich-run'
import { simpleRun, type SimpleRunSpec } from './simple-runs'
import { sampleInsights } from './insights'

const SPECS: SimpleRunSpec[] = [
  { session: CURRENT_SESSION, index: 1, hoursAgo: 0.9, prompt: 'Why does the invoice export skip cancelled lines?', model: 'sonnet-4.6', status: 'ok', durationMs: 41000 },
  { session: CURRENT_SESSION, index: 2, hoursAgo: 0.5, prompt: 'Show me where late fees are applied', model: 'sonnet-4.6', status: 'ok', durationMs: 23000 },
  { session: '3c1e77b0', index: 1, hoursAgo: 3.2, prompt: 'Run the migrations against the test db', model: 'sonnet-4.6', status: 'failed', durationMs: 58000, failTool: true },
  { session: '3c1e77b0', index: 2, hoursAgo: 2.8, prompt: 'Add pagination to GET /invoices', model: 'sonnet-4.6', status: 'ok', durationMs: 312000, subagent: true, compacted: true },
  { session: '9b07d2e1', index: 1, hoursAgo: 22, prompt: 'Rename Customer.mail to Customer.email everywhere', model: 'haiku-4.5', status: 'ok', durationMs: 96000 },
  { session: '9b07d2e1', index: 2, hoursAgo: 20.5, prompt: 'Make the PDF renderer use the new template', model: 'sonnet-4.6', status: 'ok', durationMs: 540000, subagent: true, evalFails: true },
  { session: '9b07d2e1', index: 3, hoursAgo: 20, prompt: 'Stop, that broke the header', model: 'sonnet-4.6', status: 'cancelled', durationMs: 18000, fellBack: true },
  { session: '7d21aa04', index: 1, hoursAgo: 52, prompt: 'Plan the split of billing into ledger and invoicing', model: 'opus-4.6', status: 'ok', durationMs: 204000, subagent: true },
  { session: '7d21aa04', index: 2, hoursAgo: 71, prompt: 'Fix flaky test webhook_retries', model: 'sonnet-4.6', status: 'failed', durationMs: 388000, failTool: true, evalFails: true },
]

export function sampleReport(): Report {
  const now = Date.now()
  const sessions = new Map<string, SessionRuns>()
  const add = (runEntry: SessionRuns['runs'][number]) => {
    const entry = sessions.get(runEntry.session) ?? { id: runEntry.session, runs: [] }
    entry.runs.push(runEntry)
    sessions.set(runEntry.session, entry)
  }
  SPECS.forEach((spec) => add(simpleRun(spec, now)))
  add(richRun(now - 8 * 60_000))
  const list = [...sessions.values()].map((entry) => ({ ...entry, runs: entry.runs.sort((a, b) => a.index - b.index) }))
  return {
    project: 'billing-api',
    current: CURRENT_SESSION,
    generated_at_ms: now,
    sessions: list,
    insights: sampleInsights(list, now),
  }
}
