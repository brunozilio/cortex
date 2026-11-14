import { ChevronRight } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { CopyButton } from '@/components/CopyButton'
import { PageTitle } from '@/components/PageTitle'
import { shortId, when } from '@/lib/format'
import { CHECK_LABELS, checkVariant } from '@/lib/signals'
import { EvalSummary } from './EvalSummary'
import type { Finding, Report } from '@/types'

interface EvaluationsViewProps {
  report: Report
  onFinding: (finding: Finding) => void
}

export function EvaluationsView({ report, onFinding }: EvaluationsViewProps) {
  const { checks, findings } = report.insights
  const severeReviews = findings.filter((finding) => finding.check === 'review' && finding.severe).length
  return (
    <section className="grid gap-4">
      <PageTitle title="Evaluations" description="Checks that ran on your runs in the last 30 days." />
      <EvalSummary checks={checks} severeReviews={severeReviews} />
      <Card>
        <CardHeader>
          <CardTitle>Recent findings</CardTitle>
          <CardDescription>Open one to see the run at that span. Auto-review findings can be copied as your next prompt.</CardDescription>
        </CardHeader>
        {findings.length ? (
          <Table>
            <TableHeader>
              <TableRow className="border-t">
                <TableHead>Check</TableHead>
                <TableHead>Finding</TableHead>
                <TableHead>Run</TableHead>
                <TableHead>When</TableHead>
                <TableHead className="w-24" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {findings.map((finding, i) => (
                <TableRow
                  key={i}
                  tabIndex={0}
                  className="group cursor-pointer"
                  onClick={() => onFinding(finding)}
                  onKeyDown={(event) => event.key === 'Enter' && onFinding(finding)}
                >
                  <TableCell className="py-3">
                    <Badge variant={checkVariant(finding)}>{CHECK_LABELS[finding.check]}</Badge>
                  </TableCell>
                  <TableCell className="min-w-[260px]">{finding.text}</TableCell>
                  <TableCell className="whitespace-nowrap font-mono text-xs">
                    {shortId(finding.session)} · run {finding.run}
                  </TableCell>
                  <TableCell className="whitespace-nowrap text-muted-foreground">{when(finding.at_ms, report.generated_at_ms)}</TableCell>
                  <TableCell className="py-2 pr-3">
                    <div className="flex items-center justify-end gap-1">
                      {finding.check === 'review' ? (
                        <CopyButton text={finding.text} label="Copy as next prompt" iconOnly variant="ghost" size="icon" className="text-muted-foreground" />
                      ) : null}
                      <ChevronRight className="size-4 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-foreground" />
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        ) : (
          <div className="border-t p-10 text-center text-sm text-muted-foreground">No findings in this period.</div>
        )}
      </Card>
    </section>
  )
}
