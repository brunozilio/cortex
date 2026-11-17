import { useEffect, useState } from 'react'
import type { Heading } from '@/docs/headings'
import { say } from '@/docs/i18n'
import type { Lang } from '@/docs/pages'
import { cn } from '@/lib/utils'

function useActive(ids: string[]): string {
  const [active, setActive] = useState(ids[0] ?? '')
  useEffect(() => {
    const update = () => {
      let current = ids[0] ?? ''
      for (const id of ids) {
        const element = document.getElementById(id)
        if (element && element.getBoundingClientRect().top < 120) current = id
      }
      setActive(current)
    }
    update()
    window.addEventListener('scroll', update, { passive: true })
    return () => window.removeEventListener('scroll', update)
  }, [ids])
  return active
}

export function Toc({ lang, items, onJump }: { lang: Lang; items: Heading[]; onJump: (id: string) => void }) {
  const active = useActive(items.map((item) => item.id))
  if (items.length < 2) return null
  return (
    <div className="grid gap-2 text-sm">
      <div className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">{say(lang, 'onThisPage')}</div>
      <ul className="grid gap-1 border-l">
        {items.map((item) => (
          <li key={item.id}>
            <a
              href={`#${item.id}`}
              onClick={(event) => {
                event.preventDefault()
                onJump(item.id)
              }}
              className={cn(
                '-ml-px block border-l py-0.5 leading-snug transition-colors',
                item.depth === 3 ? 'pl-6 text-[13px]' : 'pl-3',
                item.id === active ? 'border-foreground font-medium text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground',
              )}
            >
              {item.text}
            </a>
          </li>
        ))}
      </ul>
    </div>
  )
}
