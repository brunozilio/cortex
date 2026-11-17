import { useEffect, useRef, useState } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { RotateCcw } from 'lucide-react'
import { Card } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { PageTitle } from '@/components/PageTitle'
import { resizeMessage, screenSize, terminalUrl } from '@/lib/terminal'
import { useStrings } from '@/shell/strings'

type Status = 'connecting' | 'running' | 'ended'

const THEME = { background: '#0b0b0f', foreground: '#e4e4e7', cursor: '#e4e4e7', selectionBackground: '#3f3f46' }
const FONT = 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace'

export function TerminalView({ session }: { session: string }) {
  const text = useStrings().terminal
  const host = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<Status>('connecting')
  const [attempt, setAttempt] = useState(0)

  useEffect(() => {
    const element = host.current
    if (!element) return
    const terminal = new Terminal({ cursorBlink: true, fontFamily: FONT, fontSize: 13, theme: THEME, allowProposedApi: true, macOptionIsMeta: true })
    const fit = new FitAddon()
    terminal.loadAddon(fit)
    terminal.open(element)
    fit.fit()
    setStatus('connecting')

    const socket = new WebSocket(terminalUrl(window.location, session, { cols: terminal.cols, rows: terminal.rows }))
    socket.binaryType = 'arraybuffer'
    const encoder = new TextEncoder()
    socket.onopen = () => {
      setStatus('running')
      terminal.focus()
    }
    let mirrored = false
    socket.onmessage = (event: MessageEvent<ArrayBuffer | string>) => {
      if (typeof event.data !== 'string') return terminal.write(new Uint8Array(event.data))
      const size = screenSize(event.data)
      if (!size) return
      mirrored = true
      terminal.resize(size.cols, size.rows)
    }
    socket.onclose = () => setStatus('ended')
    const typing = terminal.onData((data) => {
      if (socket.readyState === WebSocket.OPEN) socket.send(encoder.encode(data))
    })
    const binary = terminal.onBinary((data) => {
      if (socket.readyState === WebSocket.OPEN) socket.send(Uint8Array.from(data, (char) => char.charCodeAt(0)))
    })
    const resizing = terminal.onResize((size) => {
      const message = resizeMessage(size)
      if (message && !mirrored && socket.readyState === WebSocket.OPEN) socket.send(message)
    })
    const observer = new ResizeObserver(() => {
      if (!mirrored) fit.fit()
    })
    observer.observe(element)

    return () => {
      observer.disconnect()
      typing.dispose()
      binary.dispose()
      resizing.dispose()
      socket.close()
      terminal.dispose()
    }
  }, [attempt, session])

  return (
    <section className="grid gap-4">
      <div className="flex items-end justify-between gap-4">
        <PageTitle title={text.title} description={text.description} />
        {status === 'ended' ? (
          <Button size="sm" onClick={() => setAttempt((count) => count + 1)}>
            <RotateCcw />
            {text.again}
          </Button>
        ) : null}
      </div>
      <Card className="overflow-auto p-2" style={{ background: THEME.background }}>
        <div ref={host} className="h-[36rem] w-full" />
      </Card>
    </section>
  )
}
