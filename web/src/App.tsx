import { useEffect, useMemo, useRef, useState } from 'react'
import { TooltipProvider } from '@/components/ui/tooltip'
import { Card } from '@/components/ui/card'
import { Header } from '@/shell/Header'
import { sessionChoices } from '@/shell/SessionSelect'
import { OverviewView } from '@/views/overview/OverviewView'
import { TracesView } from '@/views/traces/TracesView'
import { TerminalView } from '@/views/terminal/TerminalView'
import type { InspectorTab } from '@/views/traces/inspector/Inspector'
import type { Metric } from '@/lib/days'
import { EMPTY_FILTERS, type Filters } from '@/lib/filters'
import { allRuns, defaultRun, findRun, latestRun, scoped, type DeriveInsights, type RunRef } from '@/lib/report'
import { followSelection, mergeSession, readLiveConfig, refetch } from '@/lib/live'
import { useSessions, watchSession } from '@/lib/sessions'
import { useLiveFeed } from '@/lib/useLiveFeed'
import { STRINGS } from '@/lib/i18n'
import { keepLang, type Lang } from '@/lib/lang'
import { StringsProvider } from '@/shell/strings'
import type { Report, Run, SessionRuns } from '@/types'

function refOf(run: Run | undefined): RunRef | null {
  return run ? { session: run.session, index: run.index } : null
}

export function App({ initial, initialLang, sample }: { initial: Report; initialLang: Lang; sample?: DeriveInsights }) {
  const [lang, setLang] = useState(initialLang)
  const strings = STRINGS[lang]
  useEffect(() => {
    document.documentElement.lang = strings.locale
  }, [strings])
  const chooseLang = (next: Lang) => {
    setLang(next)
    keepLang(next)
  }
  const [report, setReport] = useState(initial)
  const [config] = useState(readLiveConfig)
  const cards = useSessions(config)
  const [session, setSession] = useState<string | null>(null)
  const [metric, setMetric] = useState<Metric>('cost')
  const [selected, setSelected] = useState<RunRef | null>(() => refOf(defaultRun(initial)))
  const [spanId, setSpanId] = useState(1)
  const [tab, setTab] = useState<InspectorTab>('io')
  const [filters, setFilters] = useState<Filters>(EMPTY_FILTERS)
  const latest = useRef({ report, selection: { run: selected, spanId } })
  latest.current = { report, selection: { run: selected, spanId } }

  const live = useLiveFeed({
    onSession: (update: SessionRuns) => {
      const before = latest.current.report
      const after = mergeSession(before, update)
      const next = followSelection(before, after, update, latest.current.selection)
      latest.current = { report: after, selection: next }
      setReport(after)
      setSelected(next.run)
      setSpanId(next.spanId)
    },
    onRefetch: async () => {
      const fresh = await refetch()
      latest.current = { ...latest.current, report: fresh }
      setReport(fresh)
    },
  })

  const { report: shown, ready } = useMemo(() => (session ? scoped(report, session, sample) : { report, ready: true }), [report, session, sample])
  const runs = useMemo(() => allRuns(shown), [shown])
  const choices = useMemo(() => sessionChoices(report, cards), [report, cards])
  const run = selected && (!session || selected.session === session) ? findRun(report, selected) : undefined

  const openRun = (target: Run, span = 1, nextTab: InspectorTab = 'io') => {
    if (config) void watchSession(target.session).catch(() => undefined)
    setSelected(refOf(target))
    setSpanId(span)
    setTab(nextTab)
  }

  const chooseSession = (id: string | null) => {
    setSession(id)
    if (!id) return
    if (config) void watchSession(id).catch(() => undefined)
    const newest = latestRun(report, id)
    if (newest) openRun(newest)
    else setSelected(null)
  }

  return (
    <StringsProvider value={strings}>
      <TooltipProvider delayDuration={200}>
        <div className="min-h-screen">
          <Header sessions={choices} activeSession={session} nowMs={report.generated_at_ms} live={live} lang={lang} onLang={chooseLang} onSession={chooseSession} />
          <main className="grid gap-8 px-4 py-6 md:px-6">
            {ready ? (
              <OverviewView report={shown} metric={metric} onMetric={setMetric} scope={session ? 'session' : 'all'} />
            ) : (
              <Card className="p-10 text-center text-sm text-muted-foreground">{strings.loadingSession}</Card>
            )}
            {session ? (
              <TracesView
                report={shown}
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
            {config && session ? <TerminalView key={session} session={session} /> : null}
          </main>
        </div>
      </TooltipProvider>
    </StringsProvider>
  )
}
