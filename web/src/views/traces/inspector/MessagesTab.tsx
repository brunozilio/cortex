import { Bell, Brain, Image, Sparkles, User, Wrench } from 'lucide-react'
import { CodeBlock } from '@/components/CodeBlock'
import { cn } from '@/lib/utils'
import type { Strings } from '@/lib/i18n'
import { useStrings } from '@/shell/strings'
import type { Block, Message, Run, Span } from '@/types'

export function sentMessages(run: Run, span: Span): Message[] | null {
  if (!span.request || !run.messages) return null
  return span.request.map((index) => run.messages?.[index]).filter((message): message is Message => Boolean(message))
}

function json(value: unknown): string {
  return typeof value === 'string' ? value : JSON.stringify(value, null, 2)
}

function preview(message: Message, strings: Strings): string {
  for (const block of message.blocks) {
    if ('Text' in block) return block.Text
    if ('ToolUse' in block) return `→ ${block.ToolUse.name}`
    if ('ToolResult' in block) return `← ${block.ToolResult.content}`
    if ('ToolResultRich' in block) return `← ${block.ToolResultRich.content}`
    if ('Reminder' in block) return block.Reminder
  }
  return strings.inspector.blocks(message.blocks.length)
}

function BlockView({ block }: { block: Block }) {
  const text = useStrings().inspector
  if ('Text' in block) return <p className="whitespace-pre-wrap break-words text-sm">{block.Text}</p>
  if ('Reminder' in block) return <CodeBlock label={text.reminder} text={block.Reminder} tall />
  if ('Thinking' in block)
    return (
      <div className="grid gap-1">
        <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
          <Brain className="size-3.5" />
          {text.thinking}
        </span>
        <p className="whitespace-pre-wrap break-words rounded border border-dashed p-2 text-sm italic text-muted-foreground">{block.Thinking.text}</p>
      </div>
    )
  if ('ToolUse' in block) return <CodeBlock label={`${text.toolCall} · ${block.ToolUse.name} · ${block.ToolUse.id}`} text={json(block.ToolUse.input)} tall />
  if ('ToolResult' in block)
    return <CodeBlock label={`${text.toolResult} · ${block.ToolResult.id}${block.ToolResult.is_error ? ` · ${text.error}` : ''}`} text={block.ToolResult.content} tall />
  if ('ToolResultRich' in block)
    return (
      <CodeBlock
        label={`${text.toolResult} · ${block.ToolResultRich.id}${block.ToolResultRich.is_error ? ` · ${text.error}` : ''} · ${text.images(block.ToolResultRich.images.length)}`}
        text={block.ToolResultRich.content}
        tall
      />
    )
  if ('Image' in block)
    return (
      <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
        <Image className="size-3.5" />
        {block.Image.data.startsWith('[') ? block.Image.data : block.Image.media_type}
      </span>
    )
  return <CodeBlock label={`Native · ${block.Native.origin}`} text={json(block.Native.item)} tall />
}

function MessageView({ message, index, label }: { message: Message; index: number | null; label?: string }) {
  const strings = useStrings()
  const text = strings.inspector
  const user = message.role === 'User'
  const tools = message.blocks.some((block) => 'ToolUse' in block || 'ToolResult' in block || 'ToolResultRich' in block)
  const reminders = message.blocks.some((block) => 'Reminder' in block)
  const Icon = tools ? Wrench : reminders ? Bell : user ? User : Sparkles
  return (
    <details open className={cn('group rounded-lg border', user ? 'bg-muted/30' : 'border-k-llm/20 bg-k-llm/5')}>
      <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-xs">
        <Icon className="size-3.5 shrink-0" />
        <span className="font-medium">{label ?? (user ? text.user : text.assistant)}</span>
        {index !== null ? <span className="font-mono text-muted-foreground">#{index + 1}</span> : null}
        <span className="truncate text-muted-foreground group-open:hidden">{preview(message, strings)}</span>
        <span className="ml-auto shrink-0 text-muted-foreground">{text.blocks(message.blocks.length)}</span>
      </summary>
      <div className="grid gap-3 border-t px-3 py-3">
        {message.blocks.map((block, i) => (
          <BlockView key={i} block={block} />
        ))}
      </div>
    </details>
  )
}

export function MessagesTab({ run, span }: { run: Run; span: Span }) {
  const text = useStrings().inspector
  const messages = sentMessages(run, span)
  if (!messages) return <p className="text-sm text-muted-foreground">{text.notKept}</p>
  return (
    <div className="grid gap-2">
      <span className="text-xs text-muted-foreground">{text.sent(messages.length)}</span>
      {messages.map((message, index) => (
        <MessageView key={index} message={message} index={index} />
      ))}
      {span.reply ? <MessageView message={span.reply} index={null} label={text.reply} /> : null}
    </div>
  )
}

export function SystemTab({ run, span }: { run: Run; span: Span }) {
  const strings = useStrings()
  const system = span.system_id ? run.systems?.[span.system_id] : undefined
  if (system === undefined) return <p className="text-sm text-muted-foreground">{strings.inspector.noSystem}</p>
  return <CodeBlock label={strings.inspector.systemPrompt(system.length.toLocaleString(strings.locale))} text={system} tall />
}

export function ToolsTab({ run, span }: { run: Run; span: Span }) {
  const text = useStrings().inspector
  const tools = span.tools_id ? run.tools?.[span.tools_id] : undefined
  if (!tools) return <p className="text-sm text-muted-foreground">{text.noTools}</p>
  return (
    <div className="grid gap-2">
      <span className="text-xs text-muted-foreground">{text.offered(tools.length)}</span>
      {tools.map((tool) => (
        <details key={tool.name} className="rounded-lg border">
          <summary className="flex cursor-pointer list-none items-center gap-2 px-3 py-2 text-sm">
            <Wrench className="size-3.5" />
            <span className="font-medium">{tool.name}</span>
          </summary>
          <div className="grid gap-3 border-t px-3 py-3">
            {tool.description ? <p className="whitespace-pre-wrap text-sm">{tool.description}</p> : null}
            <CodeBlock label={text.schema}text={json(tool.schema ?? {})} tall />
          </div>
        </details>
      ))}
    </div>
  )
}
