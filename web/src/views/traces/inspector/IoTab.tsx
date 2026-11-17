import { Brain, Sparkles, User } from 'lucide-react'
import { CodeBlock } from '@/components/CodeBlock'
import { CopyButton } from '@/components/CopyButton'
import { eventText, eventTitle } from '@/lib/events'
import { cn } from '@/lib/utils'
import type { Span } from '@/types'

function Bubble({ who, text, tone }: { who: string; text: string; tone: 'user' | 'model' | 'thinking' }) {
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
        {text || <span className="not-italic text-muted-foreground">empty</span>}
      </div>
    </div>
  )
}

export function IoTab({ span }: { span: Span }) {
  if (span.kind === 'llm') {
    return (
      <>
        <Bubble who="Prompt" text={span.request ? `${span.input} · open Messages to read them` : span.input} tone="user" />
        {span.thinking ? <Bubble who="Thinking" text={span.thinking} tone="thinking" /> : null}
        <Bubble who="Response" text={span.output} tone="model" />
      </>
    )
  }
  if (span.kind === 'event') return <CodeBlock label={eventTitle(span)} text={eventText(span)} />
  const copy = span.kind === 'tool' && span.failed && span.input ? <CopyButton text={span.input} label="Copy command" variant="ghost" size="sm" /> : null
  return (
    <>
      <CodeBlock label="Input" text={span.input} action={copy} />
      {span.thinking ? <Bubble who="Thinking" text={span.thinking} tone="thinking" /> : null}
      <CodeBlock label="Output" text={span.output} />
    </>
  )
}
