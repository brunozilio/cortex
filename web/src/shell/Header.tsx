import { liveNotice, type LiveStatus } from '@/lib/live'
import { SessionSelect, type SessionChoice } from './SessionSelect'

interface HeaderProps {
  sessions: SessionChoice[]
  activeSession: string | null
  nowMs: number
  live: LiveStatus
  onSession: (session: string | null) => void
}

export function Header({ sessions, activeSession, nowMs, live, onSession }: HeaderProps) {
  const notice = liveNotice(live)
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur">
      <div className="flex h-14 items-center gap-3 px-4 md:px-6">
        <SessionSelect choices={sessions} active={activeSession} nowMs={nowMs} onSession={onSession} onAll={() => onSession(null)} />
        {notice ? (
          <span role="status" className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
            <span className="h-2 w-2 animate-pulse rounded-full bg-amber-500" aria-hidden />
            {notice}
          </span>
        ) : null}
      </div>
    </header>
  )
}
