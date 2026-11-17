import { describe, expect, test } from 'bun:test'
import {
  FINDING_CHECKS,
  RUN_SIGNALS,
  RUN_STATUSES,
  SESSION_STATUSES,
  SPAN_KINDS,
  isInsights,
  isReport,
  isSessionCards,
  isSessionRuns,
  reportProblem,
  sessionCardsProblem,
  sessionRunsProblem,
} from '../src/lib/contract'
import { eventTitle } from '../src/lib/events'
import { KINDS } from '../src/lib/kinds'
import { STRINGS } from '../src/lib/i18n'
import { LANGS } from '../src/lib/lang'
import { SIGNALS } from '../src/lib/signals'
import { treeOrder } from '../src/lib/spans'
import type { Report, SessionRuns, Span } from '../src/types'
import { fixture } from './make'

const report = fixture('report.json') as Report
const update = fixture('session-update.json') as SessionRuns
const cards = fixture('sessions.json')

function changed(value: unknown, at: (string | number)[], to: unknown): unknown {
  const copy = structuredClone(value) as Record<string | number, unknown>
  let holder = copy
  for (const key of at.slice(0, -1)) holder = holder[key] as Record<string | number, unknown>
  const last = at[at.length - 1]
  if (to === undefined) delete holder[last]
  else holder[last] = to
  return copy
}

describe('what the Rust side serializes', () => {
  test('the report is a report to the page', () => {
    expect(reportProblem(report)).toBeNull()
    expect(isReport(report)).toBe(true)
    expect(isInsights(report.insights)).toBe(true)
  })

  test('a followed session is a session update to the page, with its own insights', () => {
    expect(sessionRunsProblem(update)).toBeNull()
    expect(isSessionRuns(update)).toBe(true)
    expect(isInsights(update.insights)).toBe(true)
  })

  test('the session list is a list of cards to the page', () => {
    expect(sessionCardsProblem(cards)).toBeNull()
    expect(isSessionCards(cards)).toBe(true)
  })

  test('the fixtures show every value the page has a name for, so a value Rust adds cannot go unnoticed', () => {
    const runs = [...report.sessions, update].flatMap((session) => session.runs)
    const spans = runs.flatMap((run) => run.spans)
    expect(new Set(spans.map((span) => span.kind))).toEqual(new Set(SPAN_KINDS))
    expect(new Set(runs.flatMap((run) => run.signals))).toEqual(new Set(RUN_SIGNALS))
    expect(new Set(runs.map((run) => run.status))).toEqual(new Set(RUN_STATUSES))
    expect(new Set(report.insights.findings.map((finding) => finding.check))).toEqual(new Set(FINDING_CHECKS))
    expect(new Set((cards as { status: string }[]).map((card) => card.status))).toEqual(new Set(SESSION_STATUSES))
  })

  test('every such value has its label and style in the page', () => {
    expect(Object.keys(KINDS).sort()).toEqual([...SPAN_KINDS].sort())
    expect(Object.keys(SIGNALS).sort()).toEqual([...RUN_SIGNALS].sort())
    for (const lang of LANGS) {
      const strings = STRINGS[lang]
      expect(Object.keys(strings.kinds).sort()).toEqual([...SPAN_KINDS].sort())
      expect(Object.keys(strings.signals).sort()).toEqual([...RUN_SIGNALS].sort())
      expect(Object.keys(strings.status).sort()).toEqual([...RUN_STATUSES].sort())
      expect(Object.keys(strings.checkNames).sort()).toEqual([...FINDING_CHECKS].sort())
    }
  })

  test('the fields Rust leaves out when empty are the ones the page treats as optional', () => {
    const light = report.sessions[1].runs[0]
    expect([light.messages, light.systems, light.tools]).toEqual([undefined, undefined, undefined])
    expect(light.spans.every((span) => span.request === undefined && span.reply === undefined)).toBe(true)
    const full = update.runs[0]
    expect(full.messages?.length).toBeGreaterThan(0)
    expect(Object.keys(full.systems ?? {})).toEqual(['sys-1'])
    expect(full.tools?.['tools-1']?.[0].name).toBe('Bash')
    const call = full.spans.find((span) => span.kind === 'llm' && span.request)
    expect([call?.request, call?.reply?.role, call?.system_id, call?.tools_id]).toEqual([[0], 'Assistant', 'sys-1', 'tools-1'])
  })
})

describe('what the page refuses', () => {
  test('a payload of another shape altogether', () => {
    for (const value of [null, undefined, 'report', 42, [], { error: { code: 'not_ready' } }]) {
      expect(isReport(value)).toBe(false)
      expect(isSessionRuns(value)).toBe(false)
    }
    expect(isSessionCards({})).toBe(false)
    expect(reportProblem([])).toBe('report should be an object, not a list')
  })

  test('a field that is missing, of the wrong type, or holding a value the page has no name for, said by its path', () => {
    const cases: [(string | number)[], unknown, string][] = [
      [['generated_at_ms'], '1', 'report.generated_at_ms should be a number, not "1"'],
      [['sessions', 0, 'runs', 0, 'status'], 'paused', 'report.sessions[0].runs[0].status should be one of ok, failed, cancelled, running, not "paused"'],
      [['sessions', 0, 'runs', 0, 'spans', 2, 'kind'], 'hook', 'report.sessions[0].runs[0].spans[2].kind should be one of run, llm, tool, agent, task, eval, wait, event, not "hook"'],
      [['sessions', 0, 'runs', 0, 'spans', 0, 'attrs', 0], ['model'], 'report.sessions[0].runs[0].spans[0].attrs[0] should be a pair, not a list'],
      [['sessions', 0, 'runs', 0, 'spans', 1, 'usage', 'cost_usd'], null, 'report.sessions[0].runs[0].spans[1].usage.cost_usd should be a number, not null'],
      [['sessions', 0, 'runs', 0, 'signals', 0], 'slow', 'report.sessions[0].runs[0].signals[0] should be one of'],
      [['sessions', 0, 'runs', 0, 'spans'], undefined, 'report.sessions[0].runs[0].spans should be a list, not missing'],
      [['insights', 'totals', 'runs'], undefined, 'report.insights.totals.runs should be a number, not missing'],
      [['insights', 'findings', 0, 'check'], 'lint', 'report.insights.findings[0].check should be one of judge, review, repeated, refused, not "lint"'],
      [['insights', 'time', 0, 'kind'], 'idle', 'report.insights.time[0].kind should be one of'],
      [['insights', 'days', 0, 'models'], {}, 'report.insights.days[0].models should be a list, not an object'],
    ]
    for (const [at, to, problem] of cases) {
      expect(reportProblem(changed(report, at, to))).toContain(problem)
    }
  })

  test('a session update or a card that is wrong inside', () => {
    expect(sessionRunsProblem(changed(update, ['insights', 'checks'], undefined))).toBe('update.insights.checks should be an object, not missing')
    expect(sessionRunsProblem(changed(update, ['runs', 0, 'messages', 0, 'role'], 'System'))).toContain('update.runs[0].messages[0].role should be one of User, Assistant')
    expect(sessionRunsProblem(changed(update, ['insights'], undefined))).toBeNull()
    expect(sessionCardsProblem(changed(cards, [0, 'status'], 'sleeping'))).toContain('sessions[0].status should be one of idle, running')
    expect(sessionCardsProblem(changed(cards, [1, 'title'], 7))).toBe('sessions[1].title should be text, not 7')
    expect(sessionCardsProblem(changed(cards, [0, 'cwd'], undefined))).toBeNull()
  })

  test('fields the page does not know are let through', () => {
    expect(reportProblem(changed(report, ['sessions', 0, 'runs', 0, 'later'], { any: 'thing' }))).toBeNull()
  })
})

describe('a notice in the trace', () => {
  const run = update.runs[0]
  const notice = run.spans.find((span) => span.name === 'notice') as Span

  test('arrives as an event of the run, carrying what the app told the user', () => {
    expect([notice.kind, notice.parent, notice.failed]).toEqual(['event', 1, false])
    expect(notice.output).toBe('Changes retained in worktree: /work/app/.worktrees/w1')
    expect(notice.start_ms).toBe(notice.end_ms)
  })

  test('is shown like the other notes: titled, in its place in the timeline, under the run', () => {
    expect(eventTitle(notice)).toBe('Notice')
    const placed = treeOrder(run.spans)
    const row = placed.find((entry) => entry.span.id === notice.id)
    expect(row?.depth).toBe(1)
    const events = placed.filter((entry) => entry.span.kind === 'event').map((entry) => eventTitle(entry.span))
    expect(events).toEqual(['Context compacted', 'Fell back to haiku', 'Steered', 'Notice', 'Inbox notice'])
  })

  test('every note the trace can carry has a title, and a name the page does not know is shown as it is', () => {
    const named = (name: string) => eventTitle({ ...notice, name })
    expect(['partial', 'diagnosed', 'mode', 'moved', 'notified', 'asked', 'rewound', 'failed', 'cancelled'].map(named)).toEqual([
      'Partial trace',
      'Diagnosis',
      'Mode changed',
      'Folder moved',
      'Inbox notice',
      'Question',
      'Rewound',
      'Failed',
      'Cancelled',
    ])
    expect(named('escalated to advisor')).toBe('escalated to advisor')
    expect(eventTitle({ ...notice, kind: 'tool', name: 'notice' })).toBe('notice')
  })

  test('every note has a Portuguese title too, and an unknown name is still shown as it is', () => {
    expect(Object.keys(STRINGS.pt.events).sort()).toEqual(Object.keys(STRINGS.en.events).sort())
    expect(eventTitle(notice, STRINGS.pt)).toBe('Aviso')
    expect(eventTitle({ ...notice, name: 'fell back to haiku' }, STRINGS.pt)).toBe('Fallback para haiku')
    expect(eventTitle({ ...notice, name: 'escalated to advisor' }, STRINGS.pt)).toBe('escalated to advisor')
    expect(eventTitle({ ...notice, name: 'constructor' })).toBe('constructor')
  })
})
