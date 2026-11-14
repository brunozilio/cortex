import type { Report, SessionRuns } from '@/types'
import { latestRun, runKey, type RunRef } from './report'

export type LiveStatus = 'static' | 'connecting' | 'live' | 'reconnecting'

export interface LiveConfig {
  events: string
}

export function readLiveConfig(): LiveConfig | null {
  const raw = document.getElementById('live')?.textContent?.trim()
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as Partial<LiveConfig>
    return typeof parsed.events === 'string' && parsed.events ? { events: parsed.events } : null
  } catch {
    return null
  }
}

export function mergeSession(report: Report, update: SessionRuns): Report {
  const exists = report.sessions.some((session) => session.id === update.id)
  const sessions = exists ? report.sessions.map((session) => (session.id === update.id ? update : session)) : [update, ...report.sessions]
  return { ...report, sessions }
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
