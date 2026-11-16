import { Activity, LayoutDashboard, List, ShieldCheck, SquareKanban, type LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import { shortId, usdShort, when } from '@/lib/format'
import { sessionCost } from '@/lib/report'
import { cn } from '@/lib/utils'
import type { Report } from '@/types'

export type View = 'overview' | 'sessions' | 'traces' | 'evals'

export const NAV: { key: View; label: string; icon: LucideIcon }[] = [
  { key: 'overview', label: 'Overview', icon: LayoutDashboard },
  { key: 'sessions', label: 'Sessions', icon: SquareKanban },
  { key: 'traces', label: 'Traces', icon: List },
  { key: 'evals', label: 'Evaluations', icon: ShieldCheck },
]

export function shownViews(live: boolean) {
  return NAV.filter((entry) => live || entry.key !== 'sessions')
}

interface SidebarProps {
  report: Report
  view: View
  runCount: number
  live: boolean
  activeSession: string | null
  onView: (view: View) => void
  onSession: (session: string) => void
}

function lastActivity(report: Report, session: string): number {
  return Math.max(0, ...(report.sessions.find((entry) => entry.id === session)?.runs.map((run) => run.started_at_ms + run.duration_ms) ?? []))
}

export function Sidebar({ report, view, runCount, live, activeSession, onView, onSession }: SidebarProps) {
  const severe = report.insights.findings.filter((finding) => finding.severe).length
  const sessions = [...report.sessions].sort((a, b) => (a.id === report.current ? -1 : b.id === report.current ? 1 : lastActivity(report, b.id) - lastActivity(report, a.id)))
  return (
    <aside className="sticky top-0 hidden h-screen w-60 shrink-0 flex-col gap-4 border-r bg-sidebar py-4 pl-3 pr-3 md:flex">
      <div className="flex items-center gap-2.5 px-2 py-1.5">
        <span className="grid size-8 place-items-center rounded-lg bg-primary text-primary-foreground">
          <Activity className="size-4" />
        </span>
        <span className="grid min-w-0 leading-tight">
          <span className="truncate text-sm font-semibold">{report.project}</span>
          <span className="text-xs text-muted-foreground">Trace</span>
        </span>
      </div>
      <nav className="grid gap-0.5">
        <div className="px-2 pb-1 text-xs font-medium text-muted-foreground">Observe</div>
        {shownViews(live).map(({ key, label, icon: Icon }) => (
          <button
            key={key}
            type="button"
            aria-current={view === key ? 'page' : undefined}
            onClick={() => onView(key)}
            className={cn('flex items-center gap-2.5 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent', view === key && 'bg-accent font-medium')}
          >
            <Icon className="size-4" />
            {label}
            {key === 'traces' ? <span className="ml-auto font-mono text-xs text-muted-foreground">{runCount}</span> : null}
            {key === 'evals' && severe ? (
              <Badge variant="danger" className="ml-auto">
                {severe}
              </Badge>
            ) : null}
          </button>
        ))}
      </nav>
      <div className="flex min-h-0 flex-1 flex-col gap-1">
        <div className="px-2 text-xs font-medium text-muted-foreground">Sessions</div>
        <ScrollArea className="min-h-0 flex-1">
          <div className="grid gap-0.5">
            {sessions.map((session) => {
              const isCurrent = session.id === report.current
              return (
                <button
                  key={session.id}
                  type="button"
                  onClick={() => onSession(session.id)}
                  className={cn('grid gap-0.5 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent', activeSession === session.id && 'bg-accent')}
                >
                  <span className="flex items-center gap-2.5">
                    <span className={cn('size-2 shrink-0 rounded-full', isCurrent ? 'pulse-dot bg-success' : 'bg-muted-foreground/40')} />
                    <span className="truncate font-mono text-[13px]">{shortId(session.id)}</span>
                    {isCurrent ? <span className="text-xs text-success">current</span> : null}
                    <span className="ml-auto font-mono text-xs text-muted-foreground">{usdShort(sessionCost(report, session.id))}</span>
                  </span>
                  <span className="pl-[18px] text-xs text-muted-foreground">
                    {session.runs.length} {session.runs.length === 1 ? 'run' : 'runs'} · {when(lastActivity(report, session.id), report.generated_at_ms)}
                  </span>
                </button>
              )
            })}
          </div>
        </ScrollArea>
      </div>
    </aside>
  )
}
