import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { ArrowLeft, Check, ChevronDown, Loader2, Plus, SendHorizontal, Square, X } from 'lucide-react'
import {
  CHAT_WORDS,
  ago,
  askView,
  bubbles,
  busy,
  chatApi,
  folderName,
  initials,
  pathOf,
  placeOf,
  type Bubble,
  type Chat,
  type ChatApi,
  type ChatCard,
  type ChatPlace,
  type ChatWords,
  type Decision,
  type Step,
} from '@/lib/chats'
import { preferredLang } from '@/lib/lang'
import { cn } from '@/lib/utils'

type Listener = (session: string) => void

function useChatEvents(): (listener: Listener) => () => void {
  const listeners = useRef(new Set<Listener>())
  useEffect(() => {
    const source = new EventSource('/api/events')
    const heard = (event: Event) => {
      const session = (event as MessageEvent).data as string
      for (const listener of listeners.current) listener(session)
    }
    source.addEventListener('chat', heard)
    return () => source.close()
  }, [])
  return useCallback((listener: Listener) => {
    listeners.current.add(listener)
    return () => listeners.current.delete(listener)
  }, [])
}

function useDebounced(work: () => void, ms: number): () => void {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const latest = useRef(work)
  latest.current = work
  useEffect(() => () => void (timer.current && clearTimeout(timer.current)), [])
  return useCallback(() => {
    if (timer.current) return
    timer.current = setTimeout(() => {
      timer.current = null
      latest.current()
    }, ms)
  }, [ms])
}

function Avatar({ name, busy: working }: { name: string; busy?: boolean }) {
  return (
    <div className="relative shrink-0">
      <div className="flex size-11 items-center justify-center rounded-full bg-violet-600/15 text-sm font-semibold text-violet-700 dark:text-violet-300">
        {initials(name)}
      </div>
      {working && <span className="absolute bottom-0 right-0 size-3 rounded-full border-2 border-background bg-emerald-500" />}
    </div>
  )
}

function statusLine(chat: { status: Chat['status'] }, words: ChatWords): string | null {
  if (chat.status === 'needs_you') return words.waiting
  if (busy(chat.status)) return words.working
  if (chat.status === 'failed') return words.failed
  if (chat.status === 'cancelled') return words.stopped
  return null
}

function ChatList({ api, words, go, listen }: { api: ChatApi; words: ChatWords; go: (place: ChatPlace) => void; listen: ReturnType<typeof useChatEvents> }) {
  const [cards, setCards] = useState<ChatCard[] | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const load = useCallback(() => {
    api.list().then(
      (listed) => {
        setCards(listed)
        setProblem(null)
      },
      () => setProblem(words.offline),
    )
  }, [api, words])
  const later = useDebounced(load, 800)
  useEffect(() => {
    load()
    const every = setInterval(load, 5_000)
    const stop = listen(later)
    return () => {
      clearInterval(every)
      stop()
    }
  }, [load, later, listen])
  const now = Date.now()

  return (
    <div className="flex h-dvh flex-col bg-background">
      <header className="flex items-center justify-between border-b px-4 pb-3 pt-[max(0.75rem,env(safe-area-inset-top))]">
        <h1 className="text-xl font-semibold">{words.chats}</h1>
        <a href="/trace" className="rounded-full px-3 py-1 text-sm text-muted-foreground hover:bg-muted">
          {words.trace}
        </a>
      </header>
      <main className="flex-1 overflow-y-auto">
        {problem && <p className="m-4 rounded-lg bg-amber-500/15 p-3 text-sm text-amber-800 dark:text-amber-200">{problem}</p>}
        {cards === null && !problem && <Loader2 className="mx-auto mt-10 size-6 animate-spin text-muted-foreground" />}
        {cards?.length === 0 && <p className="p-8 text-center text-sm text-muted-foreground">{words.empty}</p>}
        <ul>
          {cards?.map((card) => {
            const title = card.title || words.untitled
            const line = statusLine(card, words)
            return (
              <li key={card.session}>
                <button
                  type="button"
                  onClick={() => go({ screen: 'chat', session: card.session })}
                  className="flex w-full items-center gap-3 px-4 py-3 text-left active:bg-muted"
                >
                  <Avatar name={folderName(card.folder)} busy={busy(card.status)} />
                  <div className="min-w-0 flex-1 border-b pb-3">
                    <div className="flex items-baseline justify-between gap-2">
                      <span className="truncate font-medium">{title}</span>
                      <span className="shrink-0 text-xs text-muted-foreground">{ago(card.updated * 1000, now, words)}</span>
                    </div>
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-sm text-muted-foreground">{folderName(card.folder)}</span>
                      {line && (
                        <span className={cn('shrink-0 text-xs', card.status === 'needs_you' ? 'font-medium text-violet-600 dark:text-violet-300' : 'text-muted-foreground')}>
                          {line}
                        </span>
                      )}
                    </div>
                  </div>
                </button>
              </li>
            )
          })}
        </ul>
      </main>
      <button
        type="button"
        aria-label={words.newChat}
        onClick={() => go({ screen: 'new' })}
        className="fixed bottom-[max(1.25rem,env(safe-area-inset-bottom))] right-5 flex size-14 items-center justify-center rounded-2xl bg-violet-600 text-white shadow-lg active:scale-95"
      >
        <Plus className="size-6" />
      </button>
    </div>
  )
}

function Markdown({ text }: { text: string }) {
  return (
    <div className="break-words leading-snug [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&_a]:underline [&_code]:font-mono [&_code]:text-[13px] [&_h1]:my-1 [&_h1]:font-semibold [&_h2]:my-1 [&_h2]:font-semibold [&_h3]:my-1 [&_h3]:font-semibold [&_li]:my-0.5 [&_ol]:my-1 [&_ol]:list-decimal [&_ol]:pl-5 [&_p]:my-1 [&_pre]:my-1 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-black/5 [&_pre]:p-2 dark:[&_pre]:bg-white/10 [&_table]:block [&_table]:overflow-x-auto [&_ul]:my-1 [&_ul]:list-disc [&_ul]:pl-5">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
    </div>
  )
}

function Steps({ steps, words }: { steps: Step[]; words: ChatWords }) {
  const [open, setOpen] = useState(false)
  const last = steps[steps.length - 1]
  const mark = (step: Step) =>
    step.state === 'running' ? <Loader2 className="size-3 shrink-0 animate-spin" /> : step.state === 'failed' ? <X className="size-3 shrink-0 text-red-500" /> : <Check className="size-3 shrink-0 text-emerald-600" />
  return (
    <div className="flex justify-center">
      <button type="button" onClick={() => setOpen(!open)} className="max-w-[90%] rounded-xl bg-muted/80 px-3 py-1.5 text-left text-xs text-muted-foreground">
        {open ? (
          <ul className="space-y-1">
            {steps.map((step, index) => (
              <li key={index} className="flex items-center gap-1.5">
                {mark(step)}
                <span className="font-medium">{step.name}</span>
                <span className="truncate">{step.summary}</span>
              </li>
            ))}
          </ul>
        ) : (
          <span className="flex items-center gap-1.5">
            {mark(last)}
            <span className="font-medium">{words.steps(steps.length)}</span>
            <span className="truncate">
              · {last.name} {last.summary}
            </span>
            <ChevronDown className="size-3 shrink-0" />
          </span>
        )}
      </button>
    </div>
  )
}

function Shown({ bubble, words }: { bubble: Bubble; words: ChatWords }) {
  switch (bubble.kind) {
    case 'mine':
      return (
        <div className="flex justify-end">
          <div className="max-w-[85%] whitespace-pre-wrap break-words rounded-2xl rounded-br-md bg-violet-600 px-3 py-2 text-[15px] text-white shadow-sm">{bubble.text}</div>
        </div>
      )
    case 'theirs':
      return (
        <div className="flex justify-start">
          <div className="max-w-[92%] rounded-2xl rounded-bl-md bg-card px-3 py-2 text-[15px] shadow-sm ring-1 ring-border">
            {bubble.label && <div className="mb-1 text-xs font-semibold text-violet-600 dark:text-violet-300">{bubble.label}</div>}
            <Markdown text={bubble.text} />
          </div>
        </div>
      )
    case 'steps':
      return <Steps steps={bubble.steps} words={words} />
    case 'note':
      return (
        <div className="flex justify-center">
          <span className={cn('max-w-[90%] rounded-lg px-3 py-1 text-center text-xs', bubble.tone === 'bad' ? 'bg-red-500/10 text-red-700 dark:text-red-300' : 'bg-muted text-muted-foreground')}>
            {bubble.text}
          </span>
        </div>
      )
  }
}

function Typing() {
  return (
    <div className="flex justify-start">
      <div className="flex gap-1 rounded-2xl rounded-bl-md bg-card px-4 py-3 shadow-sm ring-1 ring-border">
        {[0, 150, 300].map((delay) => (
          <span key={delay} className="size-2 animate-bounce rounded-full bg-muted-foreground/60" style={{ animationDelay: `${delay}ms` }} />
        ))}
      </div>
    </div>
  )
}

function AskCard({ chat, words, answer }: { chat: Chat; words: ChatWords; answer: (decision: Decision) => void }) {
  if (!chat.ask) return null
  const view = askView(chat.ask, words)
  return (
    <div className="mx-3 mb-2 rounded-2xl bg-card p-3 shadow-md ring-1 ring-violet-500/40">
      <div className="text-sm font-semibold">{view.title}</div>
      <pre className="mt-1 max-h-40 overflow-auto whitespace-pre-wrap break-words rounded-md bg-muted p-2 text-xs">{view.detail}</pre>
      <div className="mt-2 flex flex-wrap gap-2">
        {view.choices.map((choice) => (
          <button
            key={choice.label}
            type="button"
            onClick={() => answer(choice.decision)}
            className={cn(
              'flex-1 rounded-full px-3 py-2 text-sm font-medium active:scale-95',
              choice.tone === 'go' ? 'bg-violet-600 text-white' : 'bg-muted text-foreground',
            )}
          >
            {choice.label}
          </button>
        ))}
      </div>
    </div>
  )
}

function Composer({ words, working, send, stop }: { words: ChatWords; working: boolean; send: (text: string) => Promise<boolean>; stop: () => void }) {
  const [text, setText] = useState('')
  const [sending, setSending] = useState(false)
  const box = useRef<HTMLTextAreaElement>(null)
  useLayoutEffect(() => {
    const area = box.current
    if (!area) return
    area.style.height = 'auto'
    area.style.height = `${Math.min(area.scrollHeight, 160)}px`
  }, [text])
  const submit = async () => {
    const said = text.trim()
    if (!said || sending || working) return
    setSending(true)
    if (await send(said)) setText('')
    setSending(false)
  }
  return (
    <div className="flex items-end gap-2 border-t bg-background/95 px-2 pb-[max(0.5rem,env(safe-area-inset-bottom))] pt-2 backdrop-blur">
      <textarea
        ref={box}
        rows={1}
        value={text}
        placeholder={words.placeholder}
        onChange={(event) => setText(event.target.value)}
        className="max-h-40 flex-1 resize-none rounded-3xl bg-muted px-4 py-2.5 text-[16px] outline-none"
      />
      {working ? (
        <button type="button" aria-label={words.stop} onClick={stop} className="flex size-11 shrink-0 items-center justify-center rounded-full bg-red-500 text-white active:scale-95">
          <Square className="size-4 fill-current" />
        </button>
      ) : (
        <button
          type="button"
          aria-label="Send"
          disabled={!text.trim() || sending}
          onClick={() => void submit()}
          className="flex size-11 shrink-0 items-center justify-center rounded-full bg-violet-600 text-white disabled:opacity-40 active:scale-95"
        >
          {sending ? <Loader2 className="size-5 animate-spin" /> : <SendHorizontal className="size-5" />}
        </button>
      )}
    </div>
  )
}

function ChatScreen({
  api,
  words,
  session,
  go,
  listen,
}: {
  api: ChatApi
  words: ChatWords
  session: string | null
  go: (place: ChatPlace, replace?: boolean) => void
  listen: ReturnType<typeof useChatEvents>
}) {
  const [chat, setChat] = useState<Chat | null>(null)
  const [pending, setPending] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const scroller = useRef<HTMLDivElement>(null)
  const pinned = useRef(true)

  const load = useCallback(() => {
    if (!session) return
    api.open(session).then(
      (opened) => {
        setChat(opened)
        setPending(null)
        setProblem(null)
      },
      (error: unknown) => setProblem(error instanceof Error ? error.message : words.offline),
    )
  }, [api, session, words])
  const later = useDebounced(load, 250)
  useEffect(() => {
    setChat(null)
    pinned.current = true
    load()
    const every = setInterval(load, 4_000)
    const stop = listen((changed) => changed === session && later())
    return () => {
      clearInterval(every)
      stop()
    }
  }, [load, later, listen, session])

  const shown = useMemo(() => (chat ? bubbles(chat.entries, words) : []), [chat, words])
  useLayoutEffect(() => {
    const area = scroller.current
    if (area && pinned.current) area.scrollTop = area.scrollHeight
  }, [shown, pending, chat?.ask])

  const working = chat ? busy(chat.status) : false
  const send = async (text: string): Promise<boolean> => {
    setPending(text)
    pinned.current = true
    try {
      if (session) {
        await api.send(session, text)
        later()
      } else {
        const started = await api.start(text)
        go({ screen: 'chat', session: started }, true)
      }
      return true
    } catch (error) {
      setPending(null)
      setProblem(error instanceof Error ? error.message : String(error))
      return false
    }
  }
  const act = (work: Promise<void>) => work.then(later, (error: unknown) => setProblem(error instanceof Error ? error.message : String(error)))
  const title = chat?.title || words.untitled
  const line = chat ? statusLine(chat, words) : null

  return (
    <div className="flex h-dvh flex-col bg-muted/40">
      <header className="flex items-center gap-2 border-b bg-background px-2 pb-2 pt-[max(0.5rem,env(safe-area-inset-top))]">
        <button type="button" aria-label="Back" onClick={() => go({ screen: 'list' })} className="rounded-full p-2 active:bg-muted">
          <ArrowLeft className="size-5" />
        </button>
        <Avatar name={title} />
        <div className="min-w-0 flex-1">
          <div className="truncate font-medium">{session ? title : words.newChat}</div>
          {line && <div className={cn('truncate text-xs', chat?.status === 'needs_you' ? 'text-violet-600 dark:text-violet-300' : 'text-muted-foreground')}>{line}</div>}
        </div>
      </header>
      <div
        ref={scroller}
        onScroll={(event) => {
          const area = event.currentTarget
          pinned.current = area.scrollHeight - area.scrollTop - area.clientHeight < 80
        }}
        className="flex-1 space-y-2 overflow-y-auto px-3 py-3"
      >
        {!session && !pending && <p className="mx-auto mt-10 max-w-xs text-center text-sm text-muted-foreground">{words.emptyChat}</p>}
        {session && !chat && !problem && <Loader2 className="mx-auto mt-10 size-6 animate-spin text-muted-foreground" />}
        {shown.map((bubble, index) => (
          <Shown key={index} bubble={bubble} words={words} />
        ))}
        {pending && <Shown bubble={{ kind: 'mine', text: pending }} words={words} />}
        {(working || pending) && !chat?.ask && <Typing />}
        {problem && <Shown bubble={{ kind: 'note', text: problem, tone: 'bad' }} words={words} />}
      </div>
      {chat?.ask && session && <AskCard chat={chat} words={words} answer={(decision) => void act(api.answer(session, chat.ask?.id ?? '', decision))} />}
      <Composer words={words} working={working} send={send} stop={() => session && void act(api.stop(session))} />
    </div>
  )
}

export function ChatApp() {
  const api = useMemo(() => chatApi((input, init) => fetch(input, init)), [])
  const words = CHAT_WORDS[preferredLang()]
  const [place, setPlace] = useState<ChatPlace>(() => placeOf(window.location.pathname) ?? { screen: 'list' })
  const listen = useChatEvents()
  useEffect(() => {
    const back = () => setPlace(placeOf(window.location.pathname) ?? { screen: 'list' })
    window.addEventListener('popstate', back)
    return () => window.removeEventListener('popstate', back)
  }, [])
  useEffect(() => {
    document.title = words.chats
  }, [words])
  const go = useCallback((next: ChatPlace, replace = false) => {
    const path = pathOf(next)
    if (replace) history.replaceState(null, '', path)
    else history.pushState(null, '', path)
    setPlace(next)
  }, [])

  if (place.screen === 'list') return <ChatList api={api} words={words} go={go} listen={listen} />
  return <ChatScreen key={place.screen === 'chat' ? place.session : 'new'} api={api} words={words} session={place.screen === 'chat' ? place.session : null} go={go} listen={listen} />
}
