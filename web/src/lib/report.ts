import type { Insights, Report, Run, SessionRuns } from '@/types'

const WAIT_MS = 250
const TRIES = 40
const NOT_READY = 503

export type DeriveInsights = (sessions: SessionRuns[], now: number) => Insights

export interface Sample {
  report: Report
  insights: DeriveInsights
}

export type Loaded =
  | { kind: 'live'; report: Report }
  | { kind: 'sample'; report: Report; insights: DeriveInsights }
  | { kind: 'failed'; status: number | null; message: string }

export function failureMessage(status: number | null): string {
  if (status === null) return 'The terminal that opened this page is not answering; it may have been closed. Run /trace again and open the link the terminal printed.'
  if (status === 401 || status === 403) return 'This page has no access to the terminal\'s data. Open it from the link the terminal printed (run /trace again for a new one).'
  if (status === NOT_READY) return 'The terminal has not sent its report yet. Check that the chat that opened this page is still running, then reload.'
  return `The page's server answered with an error (${status}). Open this page again from the link the terminal printed.`
}

async function asked(fetcher: typeof fetch): Promise<Report | number | null> {
  try {
    const response = await fetcher('/api/report')
    return response.ok ? ((await response.json()) as Report) : response.status
  } catch {
    return null
  }
}

export async function loadReport(fetcher: typeof fetch = fetch, wait = WAIT_MS, sample?: () => Promise<Sample>): Promise<Loaded> {
  let status: number | null = NOT_READY
  for (let attempt = 0; attempt < TRIES && status === NOT_READY; attempt += 1) {
    if (attempt > 0) await new Promise((resolve) => setTimeout(resolve, wait))
    const answer = await asked(fetcher)
    if (answer !== null && typeof answer === 'object') return { kind: 'live', report: answer }
    status = answer
  }
  if (sample && status !== NOT_READY) return { kind: 'sample', ...(await sample()) }
  return { kind: 'failed', status, message: failureMessage(status) }
}

export interface RunRef {
  session: string
  index: number
}

export function runKey(ref: RunRef): string {
  return `${ref.session}#${ref.index}`
}

export function allRuns(report: Report): Run[] {
  return report.sessions.flatMap((session) => session.runs).sort((a, b) => b.started_at_ms - a.started_at_ms)
}

export function findRun(report: Report, ref: RunRef): Run | undefined {
  return report.sessions.find((session) => session.id === ref.session)?.runs.find((run) => run.index === ref.index)
}

export function latestRun(report: Report, session: string): Run | undefined {
  const runs = report.sessions.find((entry) => entry.id === session)?.runs ?? []
  return runs.reduce<Run | undefined>((latest, run) => (!latest || run.started_at_ms > latest.started_at_ms ? run : latest), undefined)
}

export function defaultRun(report: Report): Run | undefined {
  return latestRun(report, report.current) ?? allRuns(report)[0]
}

export function sessionCost(report: Report, session: string): number {
  return report.sessions.find((entry) => entry.id === session)?.runs.reduce((sum, run) => sum + run.cost_usd, 0) ?? 0
}

export interface Scoped {
  report: Report
  ready: boolean
}

export function scoped(report: Report, session: string, derive?: DeriveInsights): Scoped {
  const entry = report.sessions.find((candidate) => candidate.id === session) ?? { id: session, runs: [] }
  const own = entry.insights ?? derive?.([entry], report.generated_at_ms)
  const findings = report.insights.findings.filter((finding) => finding.session === session)
  const insights = own ?? { ...report.insights, findings }
  return { report: { ...report, current: session, sessions: [entry], insights }, ready: own !== undefined }
}
