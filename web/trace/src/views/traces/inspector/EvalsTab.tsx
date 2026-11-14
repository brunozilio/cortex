import { AlertCircle, Ban, CircleCheck, Layers, ShieldCheck } from 'lucide-react'
import { Callout } from '@/components/Callout'
import { CopyButton } from '@/components/CopyButton'
import { CHECK_LABELS } from '@/lib/signals'
import { findingsFor, isRepeated, refusalReason } from '@/lib/spans'
import type { Finding, Run, Span } from '@/types'

function FindingCallout({ finding }: { finding: Finding }) {
  const tone = finding.check === 'repeated' ? 'neutral' : finding.severe ? 'danger' : 'warning'
  return (
    <Callout tone={tone} icon={finding.check === 'repeated' ? Layers : AlertCircle} title={CHECK_LABELS[finding.check]}>
      <span>{finding.text}</span>
      {finding.check === 'review' ? (
        <div>
          <CopyButton text={finding.text} label="Copy as next prompt" />
        </div>
      ) : null}
    </Callout>
  )
}

function Verdict({ span }: { span: Span }) {
  return (
    <Callout tone={span.failed ? 'danger' : 'success'} icon={span.failed ? AlertCircle : CircleCheck} title={`${span.name} ${span.failed ? 'failed' : 'passed'}`}>
      {span.output ? <span>{span.output}</span> : null}
    </Callout>
  )
}

export function EvalsTab({ run, span, findings }: { run: Run; span: Span; findings: Finding[] }) {
  const isRoot = span.parent === 0
  const scoped = findingsFor(findings, run, span)
  const withFindings = new Set(scoped.map((finding) => finding.span))
  const evals = (isRoot ? run.spans.filter((entry) => entry.kind === 'eval') : span.kind === 'eval' ? [span] : []).filter((entry) => !withFindings.has(entry.id))
  const refused = refusalReason(span)
  const blocks = [
    ...evals.map((entry) => <Verdict key={`v${entry.id}`} span={entry} />),
    ...scoped.map((finding, i) => <FindingCallout key={`f${i}`} finding={finding} />),
  ]
  if (!isRoot && span.kind !== 'eval' && span.failed) {
    blocks.push(
      <Callout key="failed" tone="danger" icon={AlertCircle} title={span.kind === 'tool' ? 'Tool error' : 'Failed'}>
        The call failed. Its time counts as waste.
      </Callout>,
    )
  }
  if (isRepeated(span)) {
    blocks.push(
      <Callout key="repeated" tone="danger" icon={Layers} title="Repeated call">
        Same call, same failure as before.
      </Callout>,
    )
  }
  if (refused) {
    blocks.push(
      <Callout key="refused" tone="danger" icon={Ban} title="Refused">
        {refused}
      </Callout>,
    )
  }
  if (!blocks.length) {
    return (
      <div className="grid place-items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
        <ShieldCheck className="size-5" />
        No check ran on this span.
      </div>
    )
  }
  return <>{blocks}</>
}
