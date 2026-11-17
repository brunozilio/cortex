import { headings, plain } from './headings'
import type { Page } from './pages'

export interface Section {
  slug: string
  page: string
  heading: string
  id: string
  text: string
}

export interface Hit {
  slug: string
  page: string
  heading: string
  id: string
  snippet: string
  score: number
}

function fold(text: string): string {
  return text.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '')
}

export function sections(page: Page): Section[] {
  const ids = headings(page.body)
  const found: Section[] = [{ slug: page.slug, page: page.title, heading: '', id: '', text: page.description }]
  let next = 0
  let fenced = false
  for (const line of page.body.split('\n')) {
    if (/^\s*(```|~~~)/.test(line)) {
      fenced = !fenced
      continue
    }
    if (!fenced && /^(##|###)\s+/.test(line) && ids[next]) {
      const heading = ids[next]
      next += 1
      found.push({ slug: page.slug, page: page.title, heading: heading.text, id: heading.id, text: '' })
      continue
    }
    const last = found[found.length - 1]
    last.text += ` ${plain(line.replace(/^[>#|\-\s]+/, ''))}`
  }
  return found.map((section) => ({ ...section, text: section.text.replace(/\s+/g, ' ').trim() }))
}

function snippet(text: string, term: string): string {
  const at = fold(text).indexOf(term)
  if (at < 0) return text.slice(0, 140)
  const start = Math.max(0, at - 50)
  return `${start > 0 ? '…' : ''}${text.slice(start, start + 160)}${start + 160 < text.length ? '…' : ''}`
}

export function search(index: Section[], query: string, limit = 12): Hit[] {
  const terms = fold(query).split(/\s+/).filter(Boolean)
  if (!terms.length) return []
  const hits: Hit[] = []
  for (const section of index) {
    const [page, heading, text] = [fold(section.page), fold(section.heading), fold(section.text)]
    if (!terms.every((term) => page.includes(term) || heading.includes(term) || text.includes(term))) continue
    const score = terms.reduce(
      (sum, term) => sum + (page.includes(term) ? 6 : 0) + (heading.includes(term) ? 10 : 0) + (text.includes(term) ? 1 : 0),
      section.heading ? 0 : 2,
    )
    hits.push({ slug: section.slug, page: section.page, heading: section.heading, id: section.id, snippet: snippet(section.text, terms[0]), score })
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, limit)
}
