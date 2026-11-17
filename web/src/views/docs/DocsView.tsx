import { useCallback, useEffect, useMemo, useState } from 'react'
import { ChevronLeft, ChevronRight, X } from 'lucide-react'
import { loadBrand, PAGES } from '@/docs/content'
import { headings } from '@/docs/headings'
import { say } from '@/docs/i18n'
import { keepLang, preferredLang } from '@/lib/lang'
import { branded, findPage, type Brand, type Lang } from '@/docs/pages'
import { parseRoute, pathOf } from '@/docs/route'
import { sections } from '@/docs/search'
import { categoryOf, neighbours, ORDER } from '@/docs/sidebar'
import { Markdown } from './Markdown'
import { Navbar } from './Navbar'
import { SearchDialog } from './SearchDialog'
import { Sidebar } from './Sidebar'
import { Toc } from './Toc'
import './docs.css'

const HOME = 'introduction'

function jumpTo(id: string) {
  if (!id) return window.scrollTo({ top: 0 })
  document.getElementById(id)?.scrollIntoView({ block: 'start' })
}

function startingPlace() {
  const route = parseRoute(window.location.pathname, window.location.search, window.location.hash)
  const lang = route.lang ?? preferredLang()
  const asked = route.query.trim().toLowerCase()
  const slug = route.slug ?? (asked && ORDER.includes(asked) ? asked : HOME)
  const searching = asked && !ORDER.includes(asked) ? route.query : null
  return { lang, slug, id: route.hash, searching }
}

export function DocsView() {
  const start = useMemo(startingPlace, [])
  const [lang, setLang] = useState<Lang>(start.lang)
  const [place, setPlace] = useState({ slug: start.slug, id: start.id })
  const [searching, setSearching] = useState<string | null>(start.searching)
  const [menu, setMenu] = useState(false)
  const [brand, setBrand] = useState<Brand>({})

  useEffect(() => {
    void loadBrand().then(setBrand)
  }, [])
  useEffect(() => {
    window.history.replaceState(null, '', pathOf(lang, place.slug, place.id))
  }, [])
  useEffect(() => {
    const back = () => {
      const route = parseRoute(window.location.pathname, window.location.search, window.location.hash)
      if (route.lang) setLang(route.lang)
      setPlace({ slug: route.slug ?? HOME, id: route.hash })
    }
    window.addEventListener('popstate', back)
    return () => window.removeEventListener('popstate', back)
  }, [])
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault()
        setSearching('')
      }
      if (event.key === '/' && !(event.target instanceof HTMLInputElement)) {
        event.preventDefault()
        setSearching('')
      }
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }, [])

  const pageIn = useCallback((slug: string) => {
    const page = findPage(PAGES, lang, slug)
    return page ? branded(page, brand) : undefined
  }, [lang, brand])
  const page = pageIn(place.slug)
  const index = useMemo(() => ORDER.flatMap((slug) => {
    const found = pageIn(slug)
    return found ? sections(found) : []
  }), [pageIn])
  const toc = useMemo(() => (page ? headings(page.body) : []), [page])

  useEffect(() => {
    document.title = page ? `${page.title} · ${brand.NAME ?? ''} ${say(lang, 'docs')}`.trim() : say(lang, 'docs')
    const timer = window.setTimeout(() => jumpTo(place.id), 30)
    return () => window.clearTimeout(timer)
  }, [page, place, brand, lang])

  const go = useCallback((slug: string, id = '') => {
    const target = slug || place.slug
    window.history.pushState(null, '', pathOf(lang, target, id))
    setPlace({ slug: target, id })
    setMenu(false)
    setSearching(null)
    if (target === place.slug) jumpTo(id)
  }, [lang, place.slug])

  const changeLang = (next: Lang) => {
    keepLang(next)
    setLang(next)
    window.history.replaceState(null, '', pathOf(next, place.slug, ''))
  }

  const { previous, next } = neighbours(place.slug)
  const translated = page?.lang === lang
  const category = categoryOf(place.slug)

  return (
    <div className="min-h-screen bg-background">
      <Navbar
        name={brand.NAME ?? ''}
        lang={lang}
        onLang={changeLang}
        onSearch={() => setSearching('')}
        onMenu={() => setMenu(true)}
        onHome={() => go(HOME)}
      />
      {menu ? (
        <div className="fixed inset-0 z-40 bg-black/40 lg:hidden" onClick={() => setMenu(false)}>
          <aside className="thin-scroll h-full w-72 overflow-y-auto border-r bg-background p-4" onClick={(event) => event.stopPropagation()}>
            <button className="mb-3 ml-auto flex" onClick={() => setMenu(false)} aria-label="close">
              <X className="size-5" />
            </button>
            <Sidebar lang={lang} current={place.slug} titleOf={pageIn} onPage={(slug) => go(slug)} />
          </aside>
        </div>
      ) : null}
      <div className="mx-auto flex max-w-[90rem]">
        <aside className="thin-scroll sticky top-14 hidden h-[calc(100vh-3.5rem)] w-72 shrink-0 overflow-y-auto border-r px-4 py-6 lg:block">
          <Sidebar lang={lang} current={place.slug} titleOf={pageIn} onPage={(slug) => go(slug)} />
        </aside>
        <main className="min-w-0 flex-1 px-4 py-8 md:px-10">
          <article className="mx-auto max-w-3xl">
            {page ? (
              <>
                {category ? <div className="mb-2 text-sm font-medium text-muted-foreground">{category.label[lang]}</div> : null}
                <h1 className="text-3xl font-bold tracking-tight md:text-4xl">{page.title}</h1>
                {page.description ? <p className="mt-3 text-lg text-muted-foreground">{page.description}</p> : null}
                {!translated ? <p className="mt-4 rounded-lg border bg-muted/40 px-4 py-2 text-sm text-muted-foreground">{say(lang, 'notTranslated')}</p> : null}
                <div className="mt-8">
                  <Markdown body={page.body} lang={lang} onLink={go} />
                </div>
                <nav className="mt-16 grid gap-4 border-t pt-8 sm:grid-cols-2">
                  {previous && pageIn(previous) ? (
                    <button onClick={() => go(previous)} className="group rounded-lg border p-4 text-left transition-colors hover:border-foreground/40">
                      <div className="text-xs text-muted-foreground">{say(lang, 'previous')}</div>
                      <div className="mt-1 flex items-center gap-1 font-medium"><ChevronLeft className="size-4" />{pageIn(previous)?.title}</div>
                    </button>
                  ) : <span />}
                  {next && pageIn(next) ? (
                    <button onClick={() => go(next)} className="group rounded-lg border p-4 text-right transition-colors hover:border-foreground/40">
                      <div className="text-xs text-muted-foreground">{say(lang, 'next')}</div>
                      <div className="mt-1 flex items-center justify-end gap-1 font-medium">{pageIn(next)?.title}<ChevronRight className="size-4" /></div>
                    </button>
                  ) : null}
                </nav>
              </>
            ) : (
              <p className="py-20 text-center text-muted-foreground">{say(lang, 'notFound')}</p>
            )}
          </article>
        </main>
        <aside className="thin-scroll sticky top-14 hidden h-[calc(100vh-3.5rem)] w-64 shrink-0 overflow-y-auto px-4 py-8 xl:block">
          <Toc lang={lang} items={toc} onJump={(id) => go(place.slug, id)} />
        </aside>
      </div>
      {searching !== null ? (
        <SearchDialog lang={lang} index={index} initial={searching} onClose={() => setSearching(null)} onPick={(slug, id) => go(slug, id)} />
      ) : null}
    </div>
  )
}
