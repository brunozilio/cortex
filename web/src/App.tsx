import { useMemo, useRef, useState } from 'react'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Header } from '@/shell/Header'
import { Sidebar, type View } from '@/shell/Sidebar'
import { sessionChoices } from '@/shell/SessionSelect'
import { OverviewView } from '@/views/overview/OverviewView'
import { TracesView } from '@/views/traces/TracesView'
import { EvaluationsView } from '@/views/evals/EvaluationsView'
import { SessionsView } from '@/views/sessions/SessionsView'
import { Composer } from '@/components/Composer'
import type { InspectorTab } from '@/views/traces/inspector/Inspector'
import type { Metric } from '@/lib/days'
import { EMPTY_FILTERS, type Filters } from '@/lib/filters'
import { allRuns, defaultRun, findRun, latestRun, type RunRef } from '@/lib/report'
import { followSelection, mergeSession, readLiveConfig } from '@/lib/live'
import { sendPrompt, useSessions, watchSession } from '@/lib/sessions'
import { useLiveFeed } from '@/lib/useLiveFeed'
import type { Finding, Report, Run, SessionRuns } from '@/types'

const VIEW_KEY = 'trace-view'

function storedView(): View {
  try {
    const value = localStorage.getItem(VIEW_KEY)
    if (value === 'overview' || value === 'sessions' || value === 'traces' || value === 'evals') return value
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
  const [config] = useState(readLiveConfig)
  const cards = useSessions(config)
  const runs = useMemo(() => allRuns(report), [report])
  const [view, setViewState] = useState<View>(storedView)
  const [metric, setMetric] = useState<Metric>('cost')
  const [selected, setSelected] = useState<RunRef | null>(() => refOf(defaultRun(initial)))
  const [spanId, setSpanId] = useState(1)
  const latest = useRef({ report, selection: { run: selected, spanId } })
  latest.current = { report, selection: { run: selected, spanId } }

  useLiveFeed((update: SessionRuns) => {
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
    if (config) void watchSession(config, target.session).catch(() => undefined)
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
    if (config) void watchSession(config, session).catch(() => undefined)
    setFilters({ ...EMPTY_FILTERS, session: [session] })
    const latest = latestRun(report, session)
    if (latest) openRun(latest)
    else setSelected(null)
    setView('traces')
  }

  const activeSession = filters.session.length === 1 ? filters.session[0] : null
  const writingTo = config ? (activeSession ?? run?.session ?? null) : null

  const choices = useMemo(() => sessionChoices(report, cards), [report, cards])
  const allSessions = () => setFilters({ ...filters, session: [] })

  return (
    <TooltipProvider delayDuration={200}>
      <div className="flex min-h-screen">
        <Sidebar report={report} view={view} runCount={runs.length} live={config !== null} onView={setView} />
        <main className="min-w-0 flex-1">
          <Header
            live={config !== null}
            view={view}
            sessions={choices}
            activeSession={activeSession}
            nowMs={report.generated_at_ms}
            onView={setView}
            onSession={openSession}
            onAllSessions={allSessions}
          />
          <div className="px-4 py-6 md:px-6">
            {view === 'overview' ? <OverviewView report={report} metric={metric} onMetric={setMetric} /> : null}
            {view === 'sessions' ? <SessionsView report={report} cards={cards} active={activeSession} onOpen={openSession} /> : null}
            {view === 'traces' ? (
              <TracesView
                report={report}
                runs={runs}
                run={run}
                spanId={spanId}
                tab={tab}
                filters={filters}
                onFilters={setFilters}
                onRun={(target) => openRun(target, 1, tab)}
                onSpan={setSpanId}
                onTab={setTab}
              />
            ) : null}
            {view === 'evals' ? <EvaluationsView report={report} onFinding={openFinding} /> : null}
            {view === 'traces' && config && writingTo ? (
              <Composer
                key={writingTo}
                session={writingTo}
                title={cards.find((card) => card.session === writingTo)?.title ?? null}
                onSend={(text) => sendPrompt(config, writingTo, text)}
              />
            ) : null}
          </div>
        </main>
      </div>
    </TooltipProvider>
  )
}
