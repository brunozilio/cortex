import { Activity, LayoutDashboard, List, ShieldCheck, SquareKanban, type LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
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
  onView: (view: View) => void
}

export function Sidebar({ report, view, runCount, live, onView }: SidebarProps) {
  const severe = report.insights.findings.filter((finding) => finding.severe).length
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
    </aside>
  )
}
