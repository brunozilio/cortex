import type { SessionRuns } from '@/types'
import { sessionRunsProblem } from './contract'
import type { LiveStatus } from './live'

export interface Feed {
  status: LiveStatus
  behind: boolean
  problem: string | null
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
    return { feed: { ...feed, problem: 'The terminal sent an update that is not JSON; it was left out.' }, effects: [] }
  }
  const problem = sessionRunsProblem(update)
  if (problem) return { feed: { ...feed, problem: `The terminal sent an update this page cannot read (${problem}); it was left out.` }, effects: [] }
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
      return { feed: { ...feed, problem: `The page could not reload the trace: ${event.message}.` }, effects: [] }
  }
}

export function feedNotice(feed: Feed): string | null {
  if (feed.status === 'reconnecting') return 'Disconnected from the terminal; what you see may be out of date. Reconnecting…'
  if (feed.behind) return `Some updates were missed; what you see is partial until the page catches up.${feed.problem ? ` ${feed.problem}` : ''}`
  return feed.problem
}
