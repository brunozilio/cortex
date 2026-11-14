import { KINDS } from '@/lib/kinds'
import { spanById } from '@/lib/spans'
import type { Run, Span } from '@/types'

export function MetadataTab({ run, span }: { run: Run; span: Span }) {
  const parent = span.parent ? spanById(run, span.parent) : undefined
  const rows: [string, string][] = [
    ['Kind', KINDS[span.kind].label],
    ['Span', `#${span.id}`],
    ['Parent', parent ? `${parent.name} (#${parent.id})` : '–'],
  ]
  if (span.stop) rows.push(['Stop reason', span.stop])
  if (span.usage) rows.push(['Model', span.usage.model])
  rows.push(...span.attrs)
  span.flags.forEach((flag) => rows.push(['Flag', flag]))
  return (
    <dl className="divide-y rounded-lg border text-sm">
      {rows.map(([key, value], i) => (
        <div key={`${key}-${i}`} className="flex gap-4 px-3 py-2">
          <dt className="w-28 shrink-0 text-muted-foreground">{key}</dt>
          <dd className="min-w-0 break-words font-mono text-[13px]">{value}</dd>
        </div>
      ))}
    </dl>
  )
}
