import { feedNotice, type Feed } from '@/lib/feed'
import { LANGS, type Lang } from '@/lib/lang'
import { cn } from '@/lib/utils'
import { SessionSelect, type SessionChoice } from './SessionSelect'
import { useStrings } from './strings'

interface HeaderProps {
  sessions: SessionChoice[]
  activeSession: string | null
  nowMs: number
  live: Feed
  lang: Lang
  onLang: (lang: Lang) => void
  onSession: (session: string | null) => void
}

export function Header({ sessions, activeSession, nowMs, live, lang, onLang, onSession }: HeaderProps) {
  const strings = useStrings()
  const notice = feedNotice(live, strings)
  return (
    <header className="sticky top-0 z-20 border-b bg-background/80 backdrop-blur">
      <div className="flex h-14 items-center gap-3 px-4 md:px-6">
        <SessionSelect choices={sessions} active={activeSession} nowMs={nowMs} onSession={onSession} onAll={() => onSession(null)} />
        {notice ? (
          <span role="status" className="ml-auto flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
            <span className="h-2 w-2 shrink-0 animate-pulse rounded-full bg-amber-500" aria-hidden />
            {notice}
          </span>
        ) : null}
        <div role="radiogroup" aria-label={strings.language} className={cn('flex shrink-0 overflow-hidden rounded-lg border text-xs font-medium', !notice && 'ml-auto')}>
          {LANGS.map((option) => (
            <button
              key={option}
              type="button"
              role="radio"
              aria-checked={option === lang}
              onClick={() => onLang(option)}
              className={cn('px-2.5 py-1.5 uppercase', option === lang ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent')}
            >
              {option}
            </button>
          ))}
        </div>
      </div>
    </header>
  )
}
