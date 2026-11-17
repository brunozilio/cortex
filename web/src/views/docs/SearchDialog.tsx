import { useEffect, useMemo, useRef, useState } from 'react'
import { CornerDownLeft, FileText, Hash, Search } from 'lucide-react'
import { say } from '@/docs/i18n'
import type { Lang } from '@/docs/pages'
import { search, type Section } from '@/docs/search'
import { cn } from '@/lib/utils'

interface SearchDialogProps {
  lang: Lang
  index: Section[]
  initial: string
  onClose: () => void
  onPick: (slug: string, id: string) => void
}

export function SearchDialog({ lang, index, initial, onClose, onPick }: SearchDialogProps) {
  const [query, setQuery] = useState(initial)
  const [chosen, setChosen] = useState(0)
  const list = useRef<HTMLUListElement>(null)
  const hits = useMemo(() => search(index, query), [index, query])

  useEffect(() => setChosen(0), [query])
  useEffect(() => {
    list.current?.querySelector(`[data-hit="${chosen}"]`)?.scrollIntoView({ block: 'nearest' })
  }, [chosen])

  const pick = (at: number) => {
    const hit = hits[at]
    if (hit) onPick(hit.slug, hit.id)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 p-4 pt-[10vh] backdrop-blur-sm" onClick={onClose}>
      <div className="w-full max-w-2xl overflow-hidden rounded-xl border bg-popover shadow-2xl" onClick={(event) => event.stopPropagation()}>
        <div className="flex items-center gap-3 border-b px-4">
          <Search className="size-4 text-muted-foreground" />
          <input
            autoFocus
            className="h-12 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
            placeholder={say(lang, 'searchHint')}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') onClose()
              if (event.key === 'ArrowDown') setChosen((at) => Math.min(at + 1, hits.length - 1))
              if (event.key === 'ArrowUp') setChosen((at) => Math.max(at - 1, 0))
              if (event.key === 'Enter') pick(chosen)
              if (['ArrowDown', 'ArrowUp', 'Enter'].includes(event.key)) event.preventDefault()
            }}
          />
          <kbd className="rounded border px-1.5 py-0.5 text-[10px] text-muted-foreground">Esc</kbd>
        </div>
        <ul ref={list} className="thin-scroll max-h-[60vh] overflow-y-auto p-2">
          {query.trim() && hits.length === 0 ? (
            <li className="px-3 py-10 text-center text-sm text-muted-foreground">
              {say(lang, 'noResults')} “{query}”
            </li>
          ) : null}
          {hits.map((hit, at) => (
            <li key={`${hit.slug}#${hit.id}`} data-hit={at}>
              <button
                className={cn('flex w-full items-start gap-3 rounded-lg px-3 py-2.5 text-left', at === chosen ? 'bg-accent' : 'hover:bg-accent/60')}
                onMouseEnter={() => setChosen(at)}
                onClick={() => pick(at)}
              >
                {hit.heading ? <Hash className="mt-0.5 size-4 shrink-0 text-muted-foreground" /> : <FileText className="mt-0.5 size-4 shrink-0 text-muted-foreground" />}
                <span className="grid min-w-0 gap-0.5">
                  <span className="truncate text-sm font-medium">{hit.heading || hit.page}</span>
                  {hit.heading ? <span className="truncate text-xs text-muted-foreground">{hit.page}</span> : null}
                  {hit.snippet ? <span className="line-clamp-2 text-xs text-muted-foreground">{hit.snippet}</span> : null}
                </span>
                {at === chosen ? <CornerDownLeft className="ml-auto mt-0.5 size-4 shrink-0 text-muted-foreground" /> : null}
              </button>
            </li>
          ))}
        </ul>
        <div className="flex gap-4 border-t px-4 py-2 text-[11px] text-muted-foreground">
          <span>↑↓ {say(lang, 'navigate')}</span>
          <span>↵ {say(lang, 'open')}</span>
          <span>Esc {say(lang, 'close')}</span>
        </div>
      </div>
    </div>
  )
}
