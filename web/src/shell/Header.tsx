import { SessionSelect, type SessionChoice } from './SessionSelect'

interface HeaderProps {
  sessions: SessionChoice[]
  activeSession: string | null
  nowMs: number
  onSession: (session: string | null) => void
}

export function Header({ sessions, activeSession, nowMs, onSession }: HeaderProps) {
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur">
      <div className="flex h-14 items-center gap-3 px-4 md:px-6">
        <SessionSelect choices={sessions} active={activeSession} nowMs={nowMs} onSession={onSession} onAll={() => onSession(null)} />
      </div>
    </header>
  )
}
