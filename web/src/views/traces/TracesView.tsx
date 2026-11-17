import { Card } from '@/components/ui/card'
import { PageTitle } from '@/components/PageTitle'
import { RunsTable } from './RunsTable'
import { RunHeader } from './RunHeader'
import { Legend, Waterfall } from './Waterfall'
import { Inspector, type InspectorTab } from './inspector/Inspector'
import type { Filters } from '@/lib/filters'
import type { RunRef } from '@/lib/report'
import { rootSpan, spanById } from '@/lib/spans'
import { useStrings } from '@/shell/strings'
import type { Report, Run } from '@/types'

interface TracesViewProps {
  report: Report
  runs: Run[]
  run: Run | undefined
  spanId: number
  tab: InspectorTab
  filters: Filters
  onFilters: (filters: Filters) => void
  onRun: (run: Run) => void
  onSpan: (id: number) => void
  onTab: (tab: InspectorTab) => void
}

export function TracesView(props: TracesViewProps) {
  const text = useStrings().traces
  const { report, runs, run, spanId, tab } = props
  const selected: RunRef | null = run ? { session: run.session, index: run.index } : null
  const span = run ? (spanById(run, spanId) ?? rootSpan(run)) : undefined
  return (
    <section className="grid gap-4">
      <PageTitle title={text.title} description={text.description} />
      <RunsTable
        runs={runs}
        current={report.current}
        nowMs={report.generated_at_ms}
        selected={selected}
        filters={props.filters}
        onFilters={props.onFilters}
        onSelect={props.onRun}
      />
      {run && span ? (
        <div className="grid items-start gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)]">
          <Card>
            <RunHeader run={run} current={report.current} nowMs={report.generated_at_ms} />
            <Legend />
            <Waterfall run={run} selected={span.id} onSelect={props.onSpan} />
          </Card>
          <div className="min-w-0 xl:sticky xl:top-20">
            <Inspector run={run} span={span} tab={tab} findings={report.insights.findings} onTab={props.onTab} />
          </div>
        </div>
      ) : (
        <Card className="p-10 text-center text-sm text-muted-foreground">{text.none}</Card>
      )}
    </section>
  )
}
