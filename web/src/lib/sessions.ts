import { useEffect, useState } from 'react'
import type { LiveConfig } from './live'

export type Status = 'idle' | 'running' | 'thinking' | 'needs_you' | 'done' | 'failed' | 'cancelled'

export interface SessionCard {
  session: string
  title: string | null
  status: Status
  updated: number
  current: boolean
}

export type Column = 'need' | 'run' | 'done' | 'failed'

export const COLUMNS: { key: Column; label: string }[] = [
  { key: 'need', label: 'Needs you' },
  { key: 'run', label: 'Running' },
  { key: 'done', label: 'Done' },
  { key: 'failed', label: 'Failed' },
]

export function columnOf(status: Status): Column {
  if (status === 'needs_you') return 'need'
  if (status === 'running' || status === 'thinking') return 'run'
  if (status === 'failed') return 'failed'
  return 'done'
}

export function served(_config: LiveConfig, path: string): string {
  return `/api${path}`
}

async function posted(config: LiveConfig, path: string, body: object): Promise<void> {
  const response = await fetch(served(config, path), { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  if (!response.ok) throw new Error(`the terminal refused it (${response.status})`)
}

export function sendPrompt(config: LiveConfig, session: string, text: string): Promise<void> {
  return posted(config, '/prompt', { session, text })
}

export function watchSession(config: LiveConfig, session: string): Promise<void> {
  return posted(config, '/watch', { session })
}

const EVERY_MS = 2000

export function useSessions(config: LiveConfig | null): SessionCard[] {
  const [cards, setCards] = useState<SessionCard[]>([])
  useEffect(() => {
    if (!config) return
    let stopped = false
    const load = async () => {
      try {
        const response = await fetch(served(config, '/sessions'))
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
