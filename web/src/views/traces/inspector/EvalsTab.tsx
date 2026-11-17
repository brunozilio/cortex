import { AlertCircle, Ban, CircleCheck, Layers, ShieldCheck } from 'lucide-react'
import { Callout } from '@/components/Callout'
import { CopyButton } from '@/components/CopyButton'
import { useStrings } from '@/shell/strings'
import { findingsFor, isRepeated, refusalReason } from '@/lib/spans'
import type { Finding, Run, Span } from '@/types'

function FindingCallout({ finding }: { finding: Finding }) {
  const strings = useStrings()
  const tone = finding.check === 'repeated' ? 'neutral' : finding.severe ? 'danger' : 'warning'
  return (
    <Callout tone={tone} icon={finding.check === 'repeated' ? Layers : AlertCircle} title={strings.checkNames[finding.check]}>
      <span>{finding.text}</span>
      {finding.check === 'review' ? (
        <div>
          <CopyButton text={finding.text} label={strings.evals.copyPrompt} />
        </div>
      ) : null}
    </Callout>
  )
}

function Verdict({ span }: { span: Span }) {
  const text = useStrings().evals
  return (
    <Callout tone={span.failed ? 'danger' : 'success'} icon={span.failed ? AlertCircle : CircleCheck} title={`${span.name} ${span.failed ? text.failed : text.passed}`}>
      {span.output ? <span>{span.output}</span> : null}
    </Callout>
  )
}

export function EvalsTab({ run, span, findings }: { run: Run; span: Span; findings: Finding[] }) {
  const text = useStrings().evals
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
      <Callout key="failed" tone="danger" icon={AlertCircle} title={span.kind === 'tool' ? text.toolError : text.failedTitle}>
        {text.failedDetail}
      </Callout>,
    )
  }
  if (isRepeated(span)) {
    blocks.push(
      <Callout key="repeated" tone="danger" icon={Layers} title={text.repeated}>
        {text.repeatedDetail}
      </Callout>,
    )
  }
  if (refused) {
    blocks.push(
      <Callout key="refused" tone="danger" icon={Ban} title={text.refused}>
        {refused}
      </Callout>,
    )
  }
  if (!blocks.length) {
    return (
      <div className="grid place-items-center gap-2 rounded-lg border border-dashed p-8 text-center text-sm text-muted-foreground">
        <ShieldCheck className="size-5" />
        {text.none}
      </div>
    )
  }
  return <>{blocks}</>
}
