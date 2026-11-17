import { useMemo, useState } from 'react'
import { Check, ChevronsUpDown, Search } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { ScrollArea } from '@/components/ui/scroll-area'
import { shortId, when } from '@/lib/format'
import type { SessionCard } from '@/lib/sessions'
import { cn } from '@/lib/utils'
import type { Report } from '@/types'
import { useStrings } from './strings'

export interface SessionChoice {
  id: string
  title: string
  folder: string
  runs: number
  last: number
  current: boolean
}

function lastActivity(report: Report, session: string): number {
  const runs = report.sessions.find((entry) => entry.id === session)?.runs ?? []
  return Math.max(0, ...runs.map((run) => run.started_at_ms + run.duration_ms))
}

export function sessionChoices(report: Report, cards: SessionCard[]): SessionChoice[] {
  const byId = new Map<string, SessionChoice>()
  for (const card of cards) {
    const runs = report.sessions.find((entry) => entry.id === card.session)?.runs.length ?? 0
    byId.set(card.session, {
      id: card.session,
      title: card.title || shortId(card.session),
      folder: card.cwd ?? '',
      runs,
      last: Math.max(card.updated, lastActivity(report, card.session)),
      current: card.current,
    })
  }
  for (const session of report.sessions) {
    if (byId.has(session.id)) continue
    const first = session.runs[0]
    byId.set(session.id, {
      id: session.id,
      title: first?.prompt.split('\n')[0] || shortId(session.id),
      folder: '',
      runs: session.runs.length,
      last: lastActivity(report, session.id),
      current: session.id === report.current,
    })
  }
  return [...byId.values()].sort((a, b) => b.last - a.last)
}

export function matchesChoice(choice: SessionChoice, query: string): boolean {
  const needle = query.trim().toLowerCase()
  return !needle || [choice.title, choice.id, choice.folder].some((text) => text.toLowerCase().includes(needle))
}

interface SessionSelectProps {
  choices: SessionChoice[]
  active: string | null
  nowMs: number
  onSession: (session: string) => void
  onAll: () => void
}

export function SessionSelect({ choices, active, nowMs, onSession, onAll }: SessionSelectProps) {
  const strings = useStrings()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const shown = useMemo(() => choices.filter((choice) => matchesChoice(choice, query)), [choices, query])
  const chosen = choices.find((choice) => choice.id === active)
  const pick = (session: string | null) => {
    if (session) onSession(session)
    else onAll()
    setOpen(false)
    setQuery('')
  }
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          className="flex h-9 min-w-0 max-w-[min(32rem,60vw)] items-center gap-2 rounded-md border bg-background px-3 text-left text-sm shadow-sm hover:bg-accent"
        >
          <span className="truncate font-medium">{chosen ? chosen.title : strings.sessions.all}</span>
          {chosen ? <span className="shrink-0 font-mono text-xs text-muted-foreground">{shortId(chosen.id)}</span> : null}
          <ChevronsUpDown className="ml-auto size-4 shrink-0 text-muted-foreground" />
        </button>
      </PopoverTrigger>
      <PopoverContent className="w-[min(32rem,90vw)] p-0">
        <label className="relative block border-b">
          <Search className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            autoFocus
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={strings.sessions.search}
            className="h-10 rounded-none border-0 pl-9 shadow-none focus-visible:ring-0"
          />
        </label>
        <ScrollArea className="max-h-96">
          <div className="grid gap-0.5 p-1">
            {!query.trim() ? (
              <Option selected={active === null} onClick={() => pick(null)} title={strings.sessions.all} detail={strings.sessions.count(choices.length)} />
            ) : null}
            {shown.map((choice) => (
              <Option
                key={choice.id}
                selected={active === choice.id}
                onClick={() => pick(choice.id)}
                title={choice.title}
                badge={choice.current ? strings.sessions.current : undefined}
                detail={`${shortId(choice.id)} · ${strings.sessions.runs(choice.runs)}${choice.last ? ` · ${when(choice.last, nowMs, strings)}` : ''}${choice.folder ? ` · ${choice.folder}` : ''}`}
              />
            ))}
            {!shown.length ? <p className="px-3 py-6 text-center text-sm text-muted-foreground">{strings.sessions.none}</p> : null}
          </div>
        </ScrollArea>
      </PopoverContent>
    </Popover>
  )
}

function Option({ selected, onClick, title, detail, badge }: { selected: boolean; onClick: () => void; title: string; detail: string; badge?: string }) {
  return (
    <button type="button" onClick={onClick} className={cn('flex items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent', selected && 'bg-accent')}>
      <Check className={cn('mt-0.5 size-4 shrink-0', selected ? 'opacity-100' : 'opacity-0')} />
      <span className="grid min-w-0 gap-0.5">
        <span className="flex items-center gap-2 text-sm">
          <span className="truncate">{title}</span>
          {badge ? <span className="shrink-0 text-xs text-success">{badge}</span> : null}
        </span>
        <span className="truncate text-xs text-muted-foreground">{detail}</span>
      </span>
    </button>
  )
}
