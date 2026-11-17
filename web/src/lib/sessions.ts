import { useEffect, useState } from 'react'
import type { LiveConfig } from './live'

export type Status = 'idle' | 'running' | 'thinking' | 'needs_you' | 'done' | 'failed' | 'cancelled'

export interface SessionCard {
  session: string
  title: string | null
  status: Status
  updated: number
  current: boolean
  cwd?: string
}

export function served(path: string): string {
  return `/api${path}`
}

async function posted(path: string, body: object): Promise<void> {
  const response = await fetch(served(path), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  if (!response.ok) throw new Error(`the terminal refused it (${response.status})`)
}

export function watchSession(session: string): Promise<void> {
  return posted('/watch', { session })
}

const EVERY_MS = 2000

export function useSessions(config: LiveConfig | null): SessionCard[] {
  const [cards, setCards] = useState<SessionCard[]>([])
  useEffect(() => {
    if (!config) return
    let stopped = false
    const load = async () => {
      try {
        const response = await fetch(served('/sessions'))
        if (response.ok && !stopped) setCards((await response.json()) as SessionCard[])
      } catch {
        return
      }
    }
    void load()
    const timer = window.setInterval(load, EVERY_MS)
    return () => {
      stopped = true
      window.clearInterval(timer)
    }
  }, [config])
  return cards
}
