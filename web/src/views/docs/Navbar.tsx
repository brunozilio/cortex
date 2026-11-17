import { useEffect, useState } from 'react'
import { BookOpen, Menu, Moon, Search, Sun } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { say } from '@/docs/i18n'
import { LANGS, type Lang } from '@/docs/pages'
import { cn } from '@/lib/utils'

type Theme = 'light' | 'dark'

function startingTheme(): Theme {
  try {
    const kept = localStorage.getItem('docs-theme')
    if (kept === 'light' || kept === 'dark') return kept
  } catch {
    return 'light'
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

function useTheme(): [Theme, () => void] {
  const [theme, setTheme] = useState<Theme>(startingTheme)
  useEffect(() => {
    document.documentElement.dataset.theme = theme
    try {
      localStorage.setItem('docs-theme', theme)
    } catch {
      return
    }
  }, [theme])
  return [theme, () => setTheme((before) => (before === 'dark' ? 'light' : 'dark'))]
}

interface NavbarProps {
  name: string
  lang: Lang
  onLang: (lang: Lang) => void
  onSearch: () => void
  onMenu: () => void
  onHome: () => void
}

export function Navbar({ name, lang, onLang, onSearch, onMenu, onHome }: NavbarProps) {
  const [theme, toggleTheme] = useTheme()
  const mac = /mac/i.test(navigator.platform)
  return (
    <header className="sticky top-0 z-30 border-b bg-background/85 backdrop-blur">
      <div className="mx-auto flex h-14 max-w-[90rem] items-center gap-3 px-4 md:px-6">
        <Button variant="ghost" size="icon" className="lg:hidden" onClick={onMenu} aria-label={say(lang, 'menu')}>
          <Menu />
        </Button>
        <a
          href={`/docs/${lang}`}
          onClick={(event) => {
            event.preventDefault()
            onHome()
          }}
          className="flex items-center gap-2 font-semibold"
        >
          <BookOpen className="size-5" />
          <span>{name}</span>
          <span className="font-normal text-muted-foreground">{say(lang, 'docs')}</span>
        </a>
        <button
          onClick={onSearch}
          className="ml-auto flex h-9 w-full max-w-xs items-center gap-2 rounded-lg border bg-muted/40 px-3 text-sm text-muted-foreground transition-colors hover:bg-muted"
        >
          <Search className="size-4" />
          <span className="truncate">{say(lang, 'search')}</span>
          <kbd className="ml-auto hidden rounded border bg-background px-1.5 py-0.5 text-[10px] sm:inline">{mac ? '⌘' : 'Ctrl'} K</kbd>
        </button>
        <div className="flex overflow-hidden rounded-lg border text-xs font-medium">
          {LANGS.map((option) => (
            <button
              key={option}
              onClick={() => onLang(option)}
              className={cn('px-2.5 py-1.5 uppercase', option === lang ? 'bg-primary text-primary-foreground' : 'text-muted-foreground hover:bg-accent')}
            >
              {option}
            </button>
          ))}
        </div>
        <Button variant="ghost" size="icon" onClick={toggleTheme} aria-label={say(lang, 'theme')} title={say(lang, 'theme')}>
          {theme === 'dark' ? <Sun /> : <Moon />}
        </Button>
      </div>
    </header>
  )
}
