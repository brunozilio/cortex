import type { Report, Run } from '@/types'
import { sampleReport } from '@/sample'

export function loadReport(): Report {
  const raw = document.getElementById('trace')?.textContent?.trim() ?? ''
  if (!raw || raw.startsWith('{{')) return sampleReport()
  return JSON.parse(raw) as Report
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
