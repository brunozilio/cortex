import { PageTitle } from '@/components/PageTitle'
import { Badge } from '@/components/ui/badge'
import { shortId, usdShort, when } from '@/lib/format'
import { sessionCost } from '@/lib/report'
import { COLUMNS, columnOf, type Column, type SessionCard } from '@/lib/sessions'
import { cn } from '@/lib/utils'
import type { Report } from '@/types'

const DOT: Record<Column, string> = {
  need: 'bg-warning',
  run: 'pulse-dot bg-success',
  done: 'bg-muted-foreground/40',
  failed: 'bg-danger',
}

interface SessionsViewProps {
  report: Report
  cards: SessionCard[]
  active: string | null
  onOpen: (session: string) => void
}

export function SessionsView({ report, cards, active, onOpen }: SessionsViewProps) {
  const now = Date.now()
  return (
    <section className="grid gap-4">
      <PageTitle title="Sessions" description="Every session in this folder by where it stands. Open one to see its traces and write to it." />
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {COLUMNS.map(({ key, label }) => {
          const here = cards.filter((card) => columnOf(card.status) === key)
          return (
            <div key={key} className="flex min-w-0 flex-col gap-2 rounded-xl border bg-muted/40 p-2">
              <div className="flex items-center gap-2 px-1.5 py-1 text-sm font-medium">
                <span className={cn('size-2 rounded-full', DOT[key])} />
                {label}
                <span className="ml-auto font-mono text-xs text-muted-foreground">{here.length}</span>
              </div>
              {here.length === 0 ? <div className="px-1.5 pb-2 text-xs text-muted-foreground">Nothing here.</div> : null}
              {here.map((card) => {
                const runs = report.sessions.find((session) => session.id === card.session)?.runs.length
                return (
                  <button
                    key={card.session}
                    type="button"
                    onClick={() => onOpen(card.session)}
                    className={cn(
                      'grid gap-1.5 rounded-lg border bg-card p-3 text-left text-sm shadow-sm transition-colors hover:border-foreground/30',
                      active === card.session && 'ring-2 ring-ring/40',
                    )}
                  >
                    <span className="line-clamp-2 font-medium">{card.title ?? 'Untitled session'}</span>
                    <span className="flex items-center gap-2 text-xs text-muted-foreground">
                      <span className="font-mono">{shortId(card.session)}</span>
                      {card.current ? <Badge variant="success">this terminal</Badge> : null}
                      <span className="ml-auto">{when(card.updated * 1000, now)}</span>
                    </span>
                    {runs ? (
                      <span className="text-xs text-muted-foreground">
                        {runs} {runs === 1 ? 'run' : 'runs'} · {usdShort(sessionCost(report, card.session))}
                      </span>
                    ) : null}
                  </button>
                )
              })}
            </div>
          )
        })}
      </div>
    </section>
  )
}
