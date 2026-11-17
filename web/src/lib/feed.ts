import type { SessionRuns } from '@/types'
import { sessionRunsProblem } from './contract'
import { EN, type Strings } from './i18n'
import type { LiveStatus } from './live'

export type FeedProblem = { kind: 'garbled' } | { kind: 'unreadable'; detail: string } | { kind: 'refetch'; message: string }

export interface Feed {
  status: LiveStatus
  behind: boolean
  problem: FeedProblem | null
}

export type FeedEvent =
  | { type: 'open' }
  | { type: 'error' }
  | { type: 'message'; data: string }
  | { type: 'report' }
  | { type: 'resync' }
  | { type: 'caught_up' }
  | { type: 'refetch_failed'; message: string }

export type FeedEffect = { kind: 'session'; update: SessionRuns } | { kind: 'report' } | { kind: 'resync' }

export interface FeedStep {
  feed: Feed
  effects: FeedEffect[]
}

export function startFeed(live: boolean): Feed {
  return { status: live ? 'connecting' : 'static', behind: false, problem: null }
}

function received(feed: Feed, data: string): FeedStep {
  let update: unknown
  try {
    update = JSON.parse(data)
  } catch {
    return { feed: { ...feed, problem: { kind: 'garbled' } }, effects: [] }
  }
  const detail = sessionRunsProblem(update)
  if (detail) return { feed: { ...feed, problem: { kind: 'unreadable', detail } }, effects: [] }
  return { feed: feed.problem === null ? feed : { ...feed, problem: null }, effects: [{ kind: 'session', update: update as SessionRuns }] }
}

export function feedStep(feed: Feed, event: FeedEvent): FeedStep {
  switch (event.type) {
    case 'open': {
      const returning = feed.status === 'reconnecting'
      return { feed: { ...feed, status: 'live', behind: feed.behind || returning }, effects: returning ? [{ kind: 'resync' }] : [] }
    }
    case 'error':
      return { feed: { ...feed, status: 'reconnecting' }, effects: [] }
    case 'message':
      return received(feed, event.data)
    case 'report':
      return { feed, effects: [{ kind: 'report' }] }
    case 'resync':
      return { feed: { ...feed, behind: true }, effects: [{ kind: 'resync' }] }
    case 'caught_up':
      return { feed: { ...feed, behind: false, problem: null }, effects: [] }
    case 'refetch_failed':
      return { feed: { ...feed, problem: { kind: 'refetch', message: event.message } }, effects: [] }
  }
}

function problemText(problem: FeedProblem, strings: Strings): string {
  if (problem.kind === 'garbled') return strings.feed.garbled
  if (problem.kind === 'unreadable') return strings.feed.unreadable(problem.detail)
  return strings.feed.refetch(problem.message)
}

export function feedNotice(feed: Feed, strings: Strings = EN): string | null {
  if (feed.status === 'reconnecting') return strings.feed.disconnected
  const problem = feed.problem ? problemText(feed.problem, strings) : null
  if (feed.behind) return problem ? `${strings.feed.behind} ${problem}` : strings.feed.behind
  return problem
}
