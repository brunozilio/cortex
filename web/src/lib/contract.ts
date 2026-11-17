import type { Finding, Insights, Report, Run, SessionRuns, Signal, SpanKind } from '@/types'
import type { SessionCard, Status } from './sessions'

function named<Name extends string>(names: Record<Name, true>): Name[] {
  return Object.keys(names) as Name[]
}

export const SPAN_KINDS = named<SpanKind>({ run: true, llm: true, tool: true, agent: true, task: true, eval: true, wait: true, event: true })
export const RUN_SIGNALS = named<Signal>({
  tool_error: true,
  eval_failed: true,
  review: true,
  subagent: true,
  waste: true,
  repeated: true,
  fell_back: true,
  compacted: true,
  refused: true,
})
export const RUN_STATUSES = named<Run['status']>({ ok: true, failed: true, cancelled: true, running: true })
export const FINDING_CHECKS = named<Finding['check']>({ judge: true, review: true, repeated: true, refused: true })
export const SESSION_STATUSES = named<Status>({ idle: true, running: true, thinking: true, needs_you: true, done: true, failed: true, cancelled: true })

type Check = (value: unknown, path: string) => string | null

const wrong = (path: string, wanted: string, value: unknown): string => `${path} should be ${wanted}, not ${shown(value)}`

function shown(value: unknown): string {
  if (value === undefined) return 'missing'
  if (value === null) return 'null'
  if (Array.isArray(value)) return 'a list'
  if (typeof value === 'string') return JSON.stringify(value.length > 40 ? `${value.slice(0, 40)}…` : value)
  if (typeof value === 'object') return 'an object'
  return String(value)
}

const text: Check = (value, path) => (typeof value === 'string' ? null : wrong(path, 'text', value))
const flag: Check = (value, path) => (typeof value === 'boolean' ? null : wrong(path, 'true or false', value))
const count: Check = (value, path) => (typeof value === 'number' && Number.isFinite(value) ? null : wrong(path, 'a number', value))
const anything: Check = () => null

function oneOf(allowed: readonly string[]): Check {
  return (value, path) => (typeof value === 'string' && allowed.includes(value) ? null : wrong(path, `one of ${allowed.join(', ')}`, value))
}

function maybe(check: Check): Check {
  return (value, path) => (value === null ? null : check(value, path))
}

function optional(check: Check): Check {
  return (value, path) => (value === undefined ? null : check(value, path))
}

function listOf(check: Check): Check {
  return (value, path) => {
    if (!Array.isArray(value)) return wrong(path, 'a list', value)
    for (let index = 0; index < value.length; index += 1) {
      const problem = check(value[index], `${path}[${index}]`)
      if (problem) return problem
    }
    return null
  }
}

function pair(first: Check, second: Check): Check {
  return (value, path) => {
    if (!Array.isArray(value) || value.length !== 2) return wrong(path, 'a pair', value)
    return first(value[0], `${path}[0]`) ?? second(value[1], `${path}[1]`)
  }
}

function mapOf(check: Check): Check {
  return (value, path) => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return wrong(path, 'an object', value)
    for (const [key, entry] of Object.entries(value)) {
      const problem = check(entry, `${path}.${key}`)
      if (problem) return problem
    }
    return null
  }
}

function shape(fields: Record<string, Check>): Check {
  return (value, path) => {
    if (typeof value !== 'object' || value === null || Array.isArray(value)) return wrong(path, 'an object', value)
    const record = value as Record<string, unknown>
    for (const [key, check] of Object.entries(fields)) {
      const problem = check(record[key], `${path}.${key}`)
      if (problem) return problem
    }
    return null
  }
}

const usage = shape({ model: text, input: count, output: count, cache_read: count, cache_write: count, cost_usd: count })
const context = shape({ system: count, tools: count, conversation: count })
const message = shape({ role: oneOf(['User', 'Assistant']), blocks: listOf(mapOf(anything)) })
const toolSpec = shape({ name: text, description: optional(text) })

const span = shape({
  id: count,
  parent: count,
  kind: oneOf(SPAN_KINDS),
  name: text,
  start_ms: count,
  end_ms: count,
  failed: flag,
  input: text,
  output: text,
  thinking: text,
  usage: maybe(usage),
  context: maybe(context),
  stop: maybe(text),
  attrs: listOf(pair(text, text)),
  flags: listOf(text),
  request: optional(listOf(count)),
  reply: optional(message),
  system_id: optional(text),
  tools_id: optional(text),
})

const run = shape({
  session: text,
  index: count,
  prompt: text,
  started_at_ms: count,
  duration_ms: count,
  waiting_ms: count,
  model: text,
  status: oneOf(RUN_STATUSES),
  cost_usd: count,
  tokens: count,
  input: count,
  cache_read: count,
  waste: count,
  signals: listOf(oneOf(RUN_SIGNALS)),
  spans: listOf(span),
  messages: optional(listOf(message)),
  systems: optional(mapOf(text)),
  tools: optional(mapOf(listOf(toolSpec))),
})

const finding = shape({ session: text, run: count, span: count, check: oneOf(FINDING_CHECKS), text, at_ms: count, severe: flag })

const insights = shape({
  days: listOf(shape({ day: count, models: listOf(shape({ model: text, cost_usd: count, tokens: count, runs: count })) })),
  models: listOf(shape({ model: text, calls: count, p50_ms: count, p95_ms: count, cache_ratio: count, cost_usd: count })),
  tools: listOf(shape({ name: text, calls: count, failed: count, p95_ms: count, asked: count })),
  time: listOf(shape({ kind: oneOf(SPAN_KINDS), ms: count })),
  checks: shape({ verified_passed: count, verified_failed: count, review_findings: count, repeated: count, refused: count, asked: count }),
  totals: shape({ runs: count, cost_usd: count, tokens: count, waiting_ms: count, p95_run_ms: count, tool_error_rate: count, cache_ratio: count }),
  findings: listOf(finding),
})

const sessionRuns = shape({ id: text, runs: listOf(run), insights: optional(insights) })

const report = shape({ project: text, current: text, generated_at_ms: count, sessions: listOf(sessionRuns), insights })

const sessionCard = shape({
  session: text,
  title: maybe(text),
  status: oneOf(SESSION_STATUSES),
  updated: count,
  current: flag,
  cwd: optional(text),
})

export function reportProblem(value: unknown): string | null {
  return report(value, 'report')
}

export function sessionRunsProblem(value: unknown): string | null {
  return sessionRuns(value, 'update')
}

export function insightsProblem(value: unknown): string | null {
  return insights(value, 'insights')
}

export function sessionCardsProblem(value: unknown): string | null {
  return listOf(sessionCard)(value, 'sessions')
}

export function isReport(value: unknown): value is Report {
  return reportProblem(value) === null
}

export function isSessionRuns(value: unknown): value is SessionRuns {
  return sessionRunsProblem(value) === null
}

export function isInsights(value: unknown): value is Insights {
  return insightsProblem(value) === null
}

export function isSessionCards(value: unknown): value is SessionCard[] {
  return sessionCardsProblem(value) === null
}
