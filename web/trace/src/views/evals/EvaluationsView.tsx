import { Ban, Hand, Layers, MessageSquare, ShieldCheck, type LucideIcon } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table'
import { CopyButton } from '@/components/CopyButton'
import { PageTitle } from '@/components/PageTitle'
import { Ring } from '@/components/Ring'
import { shortId, when } from '@/lib/format'
import { CHECK_LABELS, checkVariant } from '@/lib/signals'
import type { Finding, Report } from '@/types'

function StatCard({ icon: Icon, label, value, description }: { icon: LucideIcon; label: string; value: number; description: string }) {
  return (
    <Card className="flex items-center gap-3 p-5">
      <span className="grid size-10 shrink-0 place-items-center rounded-lg bg-muted">
        <Icon className="size-5" />
      </span>
      <div className="grid min-w-0">
        <span className="text-sm text-muted-foreground">{label}</span>
        <span className="num text-2xl font-semibold">{value}</span>
      </div>
      <p className="ml-auto max-w-[45%] text-right text-xs text-muted-foreground">{description}</p>
    </Card>
  )
}

interface EvaluationsViewProps {
  report: Report
  onFinding: (finding: Finding) => void
}

export function EvaluationsView({ report, onFinding }: EvaluationsViewProps) {
  const { checks, findings } = report.insights
  const judged = checks.verified_passed + checks.verified_failed
  const severeReviews = findings.filter((finding) => finding.check === 'review' && finding.severe).length
  return (
    <section className="grid gap-4">
      <PageTitle title="Evaluations" description="Checks that ran on your runs in the last 30 days." />
      <div className="grid gap-4 md:grid-cols-3">
        <Card className="flex items-center gap-4 p-5">
          <Ring ratio={judged ? checks.verified_passed / judged : 0} color="hsl(var(--k-eval))" size={76} label={judged ? undefined : '—'} />
          <div className="grid min-w-0 gap-1">
            <div className="flex items-center gap-2 font-semibold">
              <ShieldCheck className="size-4 text-muted-foreground" />
              Verification judge
            </div>
            <p className="text-sm text-muted-foreground">Checks the result against the run's criteria</p>
            <div className="flex gap-1.5">
              <Badge variant="success">{checks.verified_passed} passed</Badge>
              <Badge variant={checks.verified_failed ? 'danger' : 'secondary'}>{checks.verified_failed} failed</Badge>
            </div>
          </div>
        </Card>
        <Card className="flex items-center gap-4 p-5">
          <span className="grid size-[76px] shrink-0 place-items-center rounded-full border-[7px] border-k-agent/30 text-xl font-semibold">{checks.review_findings}</span>
          <div className="grid min-w-0 gap-1">
            <div className="flex items-center gap-2 font-semibold">
              <MessageSquare className="size-4 text-muted-foreground" />
              Auto-review
            </div>
            <p className="text-sm text-muted-foreground">Reads the diff and the docs it contradicts</p>
            <div className="flex gap-1.5">
              <Badge variant={checks.review_findings ? 'warning' : 'secondary'}>{checks.review_findings} findings</Badge>
              {severeReviews ? <Badge variant="danger">{severeReviews} severe</Badge> : null}
            </div>
          </div>
        </Card>
        <StatCard icon={Layers} label="Loops stopped" value={checks.repeated} description="Same failing call repeated" />
      </div>
      <div className="grid gap-4 md:grid-cols-2">
        <StatCard icon={Ban} label="Calls refused" value={checks.refused} description="Calls the agent was not allowed to make" />
        <StatCard icon={Hand} label="Permission asks" value={checks.asked} description="Times the agent waited for you" />
      </div>
      <Card>
        <CardHeader>
          <CardTitle>Recent findings</CardTitle>
          <CardDescription>Click one to open the run at that span</CardDescription>
        </CardHeader>
        {findings.length ? (
          <Table>
            <TableHeader>
              <TableRow className="border-t">
                <TableHead>Check</TableHead>
                <TableHead>Finding</TableHead>
                <TableHead>Run</TableHead>
                <TableHead>When</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {findings.map((finding, i) => (
                <TableRow
                  key={i}
                  tabIndex={0}
                  className="cursor-pointer"
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
                  <TableCell className="text-right">
                    {finding.check === 'review' ? <CopyButton text={finding.text} label="Copy as next prompt" variant="ghost" size="sm" /> : null}
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
