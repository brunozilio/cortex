import type { Report, Run } from '@/types'
import { sampleReport } from '@/sample'

const WAIT_MS = 250
const TRIES = 40

export interface Loaded {
  report: Report
  live: boolean
}

export async function loadReport(fetcher: typeof fetch = fetch, wait = WAIT_MS): Promise<Loaded> {
  for (let attempt = 0; attempt < TRIES; attempt += 1) {
    try {
      const response = await fetcher('/api/report')
      if (response.ok) return { report: (await response.json()) as Report, live: true }
      if (response.status !== 503) break
    } catch {
      break
    }
    await new Promise((resolve) => setTimeout(resolve, wait))
  }
  return { report: sampleReport(), live: false }
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
