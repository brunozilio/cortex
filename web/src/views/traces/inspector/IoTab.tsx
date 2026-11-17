import { Brain, Sparkles, User } from 'lucide-react'
import { CodeBlock } from '@/components/CodeBlock'
import { CopyButton } from '@/components/CopyButton'
import { eventText, eventTitle } from '@/lib/events'
import { cn } from '@/lib/utils'
import { useStrings } from '@/shell/strings'
import type { Span } from '@/types'

function Bubble({ who, text, tone }: { who: string; text: string; tone: 'user' | 'model' | 'thinking' }) {
  const strings = useStrings()
  const Icon = tone === 'user' ? User : tone === 'thinking' ? Brain : Sparkles
  return (
    <div className="grid gap-1.5">
      <span className="inline-flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
        <Icon className="size-3.5" />
        {who}
      </span>
      <div
        className={cn(
          'whitespace-pre-wrap break-words rounded-lg px-3 py-2 text-sm',
          tone === 'user' && 'bg-muted',
          tone === 'model' && 'border border-k-llm/20 bg-k-llm/5',
          tone === 'thinking' && 'border border-dashed italic text-muted-foreground',
        )}
      >
        {text || <span className="not-italic text-muted-foreground">{strings.inspector.empty}</span>}
      </div>
    </div>
  )
}

export function IoTab({ span }: { span: Span }) {
  const strings = useStrings()
  const text = strings.inspector
  if (span.kind === 'llm') {
    return (
      <>
        <Bubble who={text.prompt} text={span.request ? `${span.input} · ${text.openMessages}` : span.input} tone="user" />
        {span.thinking ? <Bubble who={text.thinking} text={span.thinking} tone="thinking" /> : null}
        <Bubble who={text.response} text={span.output} tone="model" />
      </>
    )
  }
  if (span.kind === 'event') return <CodeBlock label={eventTitle(span, strings)} text={eventText(span)} />
  const copy = span.kind === 'tool' && span.failed && span.input ? <CopyButton text={span.input} label={text.copyCommand} variant="ghost" size="sm" /> : null
  return (
    <>
      <CodeBlock label={text.input} text={span.input} action={copy} />
      {span.thinking ? <Bubble who={text.thinking} text={span.thinking} tone="thinking" /> : null}
      <CodeBlock label={text.output} text={span.output} />
    </>
  )
}
