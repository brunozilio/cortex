import { useEffect, useRef, useState } from 'react'
import type { SessionRuns } from '@/types'
import { feedStep, startFeed, type Feed, type FeedEvent } from './feed'
import { readLiveConfig } from './live'

export interface FeedHandlers {
  onSession: (update: SessionRuns) => void
  onRefetch: () => Promise<void>
}

export function useLiveFeed(handlers: FeedHandlers): Feed {
  const [config] = useState(readLiveConfig)
  const [feed, setFeed] = useState<Feed>(() => startFeed(config !== null))
  const current = useRef(feed)
  const latest = useRef(handlers)
  latest.current = handlers

  useEffect(() => {
    if (!config) return
    let closed = false
    const source = new EventSource(config.events)
    const take = (event: FeedEvent) => {
      if (closed) return
      const step = feedStep(current.current, event)
      current.current = step.feed
      setFeed(step.feed)
      for (const effect of step.effects) {
        if (effect.kind === 'session') {
          latest.current.onSession(effect.update)
          continue
        }
        void latest.current
          .onRefetch()
          .then(() => take({ type: 'caught_up' }))
          .catch((error: unknown) => take({ type: 'refetch_failed', message: error instanceof Error ? error.message : String(error) }))
      }
    }
    source.onopen = () => take({ type: 'open' })
    source.onerror = () => take({ type: 'error' })
    source.addEventListener('report', () => take({ type: 'report' }))
    source.addEventListener('resync', () => take({ type: 'resync' }))
    source.onmessage = (event: MessageEvent<string>) => take({ type: 'message', data: event.data })
    return () => {
      closed = true
      source.close()
    }
  }, [config])

  return feed
}
