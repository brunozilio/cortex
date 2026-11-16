import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { shownViews, type View } from './Sidebar'
import { SessionSelect, type SessionChoice } from './SessionSelect'

interface HeaderProps {
  live: boolean
  view: View
  sessions: SessionChoice[]
  activeSession: string | null
  nowMs: number
  onView: (view: View) => void
  onSession: (session: string) => void
  onAllSessions: () => void
}

export function Header({ live, view, sessions, activeSession, nowMs, onView, onSession, onAllSessions }: HeaderProps) {
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur">
      <div className="flex h-14 items-center gap-3 px-4 md:px-6">
        <SessionSelect choices={sessions} active={activeSession} nowMs={nowMs} onSession={onSession} onAll={onAllSessions} />
      </div>
      <div className="px-4 pb-2 md:hidden">
        <Tabs value={view} onValueChange={(value) => onView(value as View)}>
          <TabsList className="w-full">
            {shownViews(live).map(({ key, label }) => (
              <TabsTrigger key={key} value={key} className="flex-1">
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      </div>
    </header>
  )
}
