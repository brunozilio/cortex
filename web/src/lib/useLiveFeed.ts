import { useEffect, useRef, useState } from 'react'
import type { SessionRuns } from '@/types'
import { readLiveConfig, type LiveStatus } from './live'

export function useLiveFeed(onSession: (update: SessionRuns) => void): LiveStatus {
  const [config] = useState(readLiveConfig)
  const [status, setStatus] = useState<LiveStatus>(config ? 'connecting' : 'static')
  const handler = useRef(onSession)
  handler.current = onSession

  useEffect(() => {
    if (!config) return
    const source = new EventSource(config.events)
    source.onopen = () => setStatus('live')
    source.onerror = () => setStatus('reconnecting')
    source.onmessage = (event: MessageEvent<string>) => {
      try {
        const update = JSON.parse(event.data) as SessionRuns
        if (update && typeof update.id === 'string' && Array.isArray(update.runs)) handler.current(update)
      } catch {
        return
      }
    }
    return () => source.close()
  }, [config])

  return status
}
