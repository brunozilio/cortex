import { useEffect, useMemo, useRef, useState } from 'react'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Header } from '@/shell/Header'
import { Sidebar, type View } from '@/shell/Sidebar'
import { OverviewView } from '@/views/overview/OverviewView'
import { TracesView } from '@/views/traces/TracesView'
import { EvaluationsView } from '@/views/evals/EvaluationsView'
import type { InspectorTab } from '@/views/traces/inspector/Inspector'
import type { Metric } from '@/lib/days'
import { EMPTY_FILTERS, type Filters } from '@/lib/filters'
import { allRuns, defaultRun, findRun, latestRun, type RunRef } from '@/lib/report'
import { followSelection, mergeSession } from '@/lib/live'
import { useLiveFeed } from '@/lib/useLiveFeed'
import type { Finding, Report, Run, SessionRuns } from '@/types'

const VIEW_KEY = 'trace-view'

function storedView(): View {
  try {
    const value = localStorage.getItem(VIEW_KEY)
    if (value === 'overview' || value === 'traces' || value === 'evals') return value
  } catch {
    return 'overview'
  }
  return 'overview'
}

function refOf(run: Run | undefined): RunRef | null {
  return run ? { session: run.session, index: run.index } : null
}

export function App({ initial }: { initial: Report }) {
  const [report, setReport] = useState(initial)
  const runs = useMemo(() => allRuns(report), [report])
  const [view, setViewState] = useState<View>(storedView)
  const [metric, setMetric] = useState<Metric>('cost')
  const [selected, setSelected] = useState<RunRef | null>(() => refOf(defaultRun(initial)))
  const [spanId, setSpanId] = useState(1)
  const latest = useRef({ report, selection: { run: selected, spanId } })
  latest.current = { report, selection: { run: selected, spanId } }

  const liveStatus = useLiveFeed((update: SessionRuns) => {
    const before = latest.current.report
    const after = mergeSession(before, update)
    const next = followSelection(before, after, update, latest.current.selection)
    latest.current = { report: after, selection: next }
    setReport(after)
    setSelected(next.run)
    setSpanId(next.spanId)
  })
  const [tab, setTab] = useState<InspectorTab>('io')
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS)
  const [query, setQuery] = useState('')
  const searchRef = useRef<HTMLInputElement>(null)
  const run = selected ? findRun(report, selected) : undefined

  const setView = (next: View) => {
    setViewState(next)
    try {
      localStorage.setItem(VIEW_KEY, next)
    } catch {
      return
    }
  }

  const openRun = (target: Run, span = 1, nextTab: InspectorTab = 'io') => {
    setSelected(refOf(target))
    setSpanId(span)
    setTab(nextTab)
  }

  const openFinding = (finding: Finding) => {
    const target = findRun(report, { session: finding.session, index: finding.run })
    if (!target) return
    openRun(target, finding.span, 'evals')
    setView('traces')
    window.scrollTo({ top: 0 })
  }

  const openSession = (session: string) => {
    setFilters({ ...EMPTY_FILTERS, session: [session] })
    const latest = latestRun(report, session)
    if (latest) openRun(latest)
    setView('traces')
  }

  const search = (value: string) => {
    setQuery(value)
    if (view !== 'traces') setView('traces')
  }

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement
      if (event.key !== '/' || target.closest('input, textarea')) return
      event.preventDefault()
      const header = searchRef.current
      if (header && header.offsetParent) return header.focus()
      setView('traces')
      window.setTimeout(() => document.getElementById('run-search')?.focus(), 0)
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [])

  return (
    <TooltipProvider delayDuration={200}>
      <div className="flex min-h-screen">
        <Sidebar
          report={report}
          view={view}
          runCount={runs.length}
          activeSession={filters.session.length === 1 ? filters.session[0] : null}
          onView={setView}
          onSession={openSession}
        />
        <main className="min-w-0 flex-1">
          <Header ref={searchRef} project={report.project} current={report.current} live={liveStatus} view={view} query={query} onView={setView} onQuery={search} />
          <div className="px-4 py-6 md:px-6">
            {view === 'overview' ? <OverviewView report={report} metric={metric} onMetric={setMetric} /> : null}
            {view === 'traces' ? (
              <TracesView
                report={report}
                runs={runs}
                run={run}
                spanId={spanId}
                tab={tab}
                filters={filters}
                query={query}
                onFilters={setFilters}
                onQuery={setQuery}
                onRun={(target) => openRun(target, 1, tab)}
                onSpan={setSpanId}
                onTab={setTab}
              />
            ) : null}
            {view === 'evals' ? <EvaluationsView report={report} onFinding={openFinding} /> : null}
          </div>
        </main>
      </div>
    </TooltipProvider>
  )
}
