import { useEffect, useState } from 'react'
import { isSessionCards } from './contract'
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

export async function loadSessions(fetcher: typeof fetch = fetch): Promise<SessionCard[] | null> {
  try {
    const response = await fetcher(served('/sessions'))
    if (!response.ok) return null
    const cards: unknown = await response.json()
    return isSessionCards(cards) ? cards : null
  } catch {
    return null
  }
}

export function useSessions(config: LiveConfig | null): SessionCard[] {
  const [cards, setCards] = useState<SessionCard[]>([])
  useEffect(() => {
    if (!config) return
    let stopped = false
    const load = async () => {
      const listed = await loadSessions()
      if (listed && !stopped) setCards(listed)
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
