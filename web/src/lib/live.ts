import type { Report, SessionRuns } from '@/types'
import { isSessionRuns, reportProblem } from './contract'
import { latestRun, runKey, type RunRef } from './report'

export type LiveStatus = 'static' | 'connecting' | 'live' | 'reconnecting'

export interface LiveConfig {
  events: string
}

export function readLiveConfig(): LiveConfig | null {
  return document.body.dataset.live === 'on' ? { events: '/api/events' } : null
}

export function mergeSession(report: Report, update: SessionRuns): Report {
  const exists = report.sessions.some((session) => session.id === update.id)
  const sessions = exists ? report.sessions.map((session) => (session.id === update.id ? update : session)) : [update, ...report.sessions]
  return { ...report, sessions }
}

async function latestUpdates(fetcher: typeof fetch): Promise<SessionRuns[]> {
  try {
    const response = await fetcher('/api/latest')
    if (!response.ok) return []
    const updates: unknown = await response.json()
    return Array.isArray(updates) ? updates.filter(isSessionRuns) : []
  } catch {
    return []
  }
}

export async function refetch(fetcher: typeof fetch = fetch): Promise<Report> {
  const response = await fetcher('/api/report')
  if (!response.ok) throw new Error(`the console answered ${response.status} for the report`)
  const report: unknown = await response.json()
  const problem = reportProblem(report)
  if (problem) throw new Error(`the console sent a report this page cannot read (${problem})`)
  const updates = await latestUpdates(fetcher)
  return updates.reduce(mergeSession, report as Report)
}

export interface Selection {
  run: RunRef | null
  spanId: number
}

export function followSelection(before: Report, after: Report, update: SessionRuns, selection: Selection): Selection {
  const previousLatest = latestRun(before, update.id)
  const following = !selection.run || (previousLatest !== undefined && runKey(previousLatest) === runKey(selection.run))
  if (!following) return selection
  const newest = latestRun(after, update.id)
  if (!newest) return selection
  const sameRun = selection.run !== null && runKey(selection.run) === runKey(newest)
  const keepSpan = sameRun && newest.spans.some((span) => span.id === selection.spanId)
  return { run: { session: newest.session, index: newest.index }, spanId: keepSpan ? selection.spanId : 1 }
}
