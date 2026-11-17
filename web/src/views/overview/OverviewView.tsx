import { KpiCards } from './KpiCards'
import { UsageChart } from './UsageChart'
import { CacheCard, ChecksCard, TimeCard } from './InsightCards'
import { ModelTable, ToolTable } from './StatTables'
import { PageTitle } from '@/components/PageTitle'
import type { Metric } from '@/lib/days'
import { useStrings } from '@/shell/strings'
import type { Report } from '@/types'

interface OverviewViewProps {
  report: Report
  metric: Metric
  onMetric: (metric: Metric) => void
  scope: 'all' | 'session'
}

export function OverviewView({ report, metric, onMetric, scope }: OverviewViewProps) {
  const strings = useStrings()
  return (
    <section className="grid gap-4">
      <PageTitle title={strings.overview.title} description={scope === 'session' ? strings.overview.session : strings.overview.all} />
      <KpiCards report={report} />
      <UsageChart report={report} metric={metric} onMetric={onMetric} />
      <div className="grid gap-4 lg:grid-cols-3">
        <TimeCard insights={report.insights} />
        <CacheCard insights={report.insights} />
        <ChecksCard insights={report.insights} />
      </div>
      <div className="grid gap-4 xl:grid-cols-2">
        <ModelTable insights={report.insights} />
        <ToolTable insights={report.insights} />
      </div>
    </section>
  )
}
