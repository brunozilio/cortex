import { useState, type KeyboardEvent } from 'react'
import { SendHorizontal } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { shortId } from '@/lib/format'

interface ComposerProps {
  session: string
  title: string | null
  onSend: (text: string) => Promise<void>
}

type State = { kind: 'idle' } | { kind: 'sending' } | { kind: 'sent' } | { kind: 'failed'; reason: string }

export function Composer({ session, title, onSend }: ComposerProps) {
  const [text, setText] = useState('')
  const [state, setState] = useState<State>({ kind: 'idle' })
  const send = async () => {
    const said = text.trim()
    if (!said || state.kind === 'sending') return
    setState({ kind: 'sending' })
    try {
      await onSend(said)
      setText('')
      setState({ kind: 'sent' })
    } catch (error) {
      setState({ kind: 'failed', reason: error instanceof Error ? error.message : String(error) })
    }
  }
  const onKey = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault()
      void send()
    }
  }
  const note =
    state.kind === 'sent'
      ? 'Sent. Its run shows up above as it goes.'
      : state.kind === 'failed'
        ? `Not sent: ${state.reason}`
        : 'Enter sends · Shift+Enter starts a new line'
  return (
    <div className="sticky bottom-0 z-10 -mx-4 border-t bg-background px-4 py-3 md:-mx-6 md:px-6">
      <div className="mx-auto grid max-w-4xl gap-1.5">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>Write to</span>
          <span className="truncate font-medium text-foreground">{title ?? 'this session'}</span>
          <span className="font-mono">{shortId(session)}</span>
        </div>
        <div className="flex items-end gap-2 rounded-xl border bg-card p-2 shadow-sm focus-within:ring-2 focus-within:ring-ring/30">
          <textarea
            value={text}
            onChange={(event) => {
              setText(event.target.value)
              if (state.kind !== 'sending') setState({ kind: 'idle' })
            }}
            onKeyDown={onKey}
            rows={Math.min(8, Math.max(2, text.split('\n').length))}
            placeholder="Ask for the next step…"
            aria-label="Message"
            className="min-h-0 flex-1 resize-none bg-transparent px-1.5 py-1 text-sm outline-none placeholder:text-muted-foreground"
          />
          <Button variant="default" size="icon" aria-label="Send" disabled={!text.trim() || state.kind === 'sending'} onClick={() => void send()}>
            <SendHorizontal />
          </Button>
        </div>
        <div className={state.kind === 'failed' ? 'text-xs text-danger' : 'text-xs text-muted-foreground'}>{note}</div>
      </div>
    </div>
  )
}
