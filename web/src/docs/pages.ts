import type { Lang } from '../lib/lang'

export { LANGS, type Lang } from '../lib/lang'

export interface Page {
  lang: Lang
  slug: string
  title: string
  description: string
  body: string
}

export type Brand = Record<string, string>

export function parsePage(lang: Lang, slug: string, raw: string): Page {
  const text = raw.replace(/\r\n/g, '\n')
  const match = /^---\n([\s\S]*?)\n---\n?/.exec(text)
  const fields: Record<string, string> = {}
  if (match) {
    for (const line of match[1].split('\n')) {
      const at = line.indexOf(':')
      if (at > 0) fields[line.slice(0, at).trim()] = line.slice(at + 1).trim().replace(/^["']|["']$/g, '')
    }
  }
  const body = match ? text.slice(match[0].length) : text
  return { lang, slug, title: fields.title ?? slug, description: fields.description ?? '', body }
}

export function fillBrand(text: string, brand: Brand): string {
  return text.replace(/\{\{([A-Z_]+)\}\}/g, (whole, key: string) => brand[key] ?? whole)
}

export function branded(page: Page, brand: Brand): Page {
  return { ...page, title: fillBrand(page.title, brand), description: fillBrand(page.description, brand), body: fillBrand(page.body, brand) }
}

export function pagesFrom(files: Record<string, string>): Map<string, Page> {
  const pages = new Map<string, Page>()
  for (const [path, raw] of Object.entries(files)) {
    const match = /\/(en|pt)\/([a-z0-9-]+)\.md$/.exec(path)
    if (!match) continue
    const lang = match[1] as Lang
    pages.set(`${lang}/${match[2]}`, parsePage(lang, match[2], raw))
  }
  return pages
}

export function findPage(pages: Map<string, Page>, lang: Lang, slug: string): Page | undefined {
  return pages.get(`${lang}/${slug}`) ?? pages.get(`en/${slug}`)
}
