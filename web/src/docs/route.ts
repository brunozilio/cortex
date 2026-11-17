import type { Lang } from './pages'

export interface Route {
  lang: Lang | null
  slug: string | null
  hash: string
  query: string
}

export function parseRoute(pathname: string, search: string, hash: string): Route {
  const parts = pathname.replace(/^\/docs\/?/, '').split('/').filter(Boolean)
  const lang = parts[0] === 'en' || parts[0] === 'pt' ? parts[0] : null
  const slug = (lang ? parts[1] : parts[0]) ?? null
  const query = new URLSearchParams(search).get('q') ?? ''
  return { lang, slug, hash: decoded(hash.replace(/^#/, '')), query }
}

function decoded(text: string): string {
  try {
    return decodeURIComponent(text)
  } catch {
    return text
  }
}

export function pathOf(lang: Lang, slug: string, id = ''): string {
  return `/docs/${lang}/${slug}${id ? `#${id}` : ''}`
}

export function internalTarget(href: string): { slug: string; id: string } | null {
  if (/^\/docs\/(?:en|pt)\/?$/.test(href)) return null
  const match = /^(?:\.\/|\/docs\/(?:en\/|pt\/)?)?([a-z0-9-]+)?(?:#(.+))?$/.exec(href)
  if (!match || href.includes('://') || (!match[1] && !match[2])) return null
  return { slug: match[1] ?? '', id: match[2] ?? '' }
}
