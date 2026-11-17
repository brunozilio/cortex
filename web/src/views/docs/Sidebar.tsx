import { useState } from 'react'
import { ChevronRight } from 'lucide-react'
import type { Lang, Page } from '@/docs/pages'
import { SIDEBAR } from '@/docs/sidebar'
import { cn } from '@/lib/utils'

interface SidebarProps {
  lang: Lang
  current: string
  titleOf: (slug: string) => Page | undefined
  onPage: (slug: string) => void
}

export function Sidebar({ lang, current, titleOf, onPage }: SidebarProps) {
  const [closed, setClosed] = useState<Set<string>>(new Set())
  const toggle = (label: string) =>
    setClosed((before) => {
      const after = new Set(before)
      if (after.has(label)) after.delete(label)
      else after.add(label)
      return after
    })

  return (
    <nav className="grid gap-1 text-sm">
      {SIDEBAR.map((category) => {
        const label = category.label[lang]
        const open = !closed.has(label) || category.slugs.includes(current)
        const pages = category.slugs.map((slug) => ({ slug, page: titleOf(slug) })).filter((entry) => entry.page)
        if (!pages.length) return null
        return (
          <div key={label} className="grid gap-0.5">
            <button
              className="flex items-center justify-between rounded-md px-2 py-1.5 text-left text-xs font-semibold uppercase tracking-wider text-muted-foreground hover:text-foreground"
              onClick={() => toggle(label)}
            >
              {label}
              <ChevronRight className={cn('size-3.5 transition-transform', open && 'rotate-90')} />
            </button>
            {open ? (
              <ul className="mb-2 grid gap-0.5 border-l pl-2 ml-2">
                {pages.map(({ slug, page }) => (
                  <li key={slug}>
                    <a
                      href={`/docs/${lang}/${slug}`}
                      onClick={(event) => {
                        event.preventDefault()
                        onPage(slug)
                      }}
                      className={cn(
                        'block rounded-md px-2 py-1.5 transition-colors',
                        slug === current ? 'bg-accent font-medium text-foreground' : 'text-muted-foreground hover:bg-accent/60 hover:text-foreground',
                      )}
                    >
                      {page?.title}
                    </a>
                  </li>
                ))}
              </ul>
            ) : null}
          </div>
        )
      })}
    </nav>
  )
}
