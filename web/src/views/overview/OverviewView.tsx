import { KpiCards } from './KpiCards'
import { UsageChart } from './UsageChart'
import { CacheCard, ChecksCard, TimeCard } from './InsightCards'
import { ModelTable, ToolTable } from './StatTables'
import { PageTitle } from '@/components/PageTitle'
import type { Metric } from '@/lib/days'
import type { Report } from '@/types'

interface OverviewViewProps {
  report: Report
  metric: Metric
  onMetric: (metric: Metric) => void
  scope: 'all' | 'session'
}

export function OverviewView({ report, metric, onMetric, scope }: OverviewViewProps) {
  return (
    <section className="grid gap-4">
      <PageTitle title="Overview" description={scope === 'session' ? 'How the agent worked in this session over the last 30 days.' : 'How the agent worked across every session over the last 30 days.'} />
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
