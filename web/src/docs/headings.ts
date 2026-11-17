export interface Heading {
  depth: 2 | 3
  text: string
  id: string
  line: number
}

export function plain(markdown: string): string {
  return markdown
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/[`*_~]/g, '')
    .trim()
}

export function slugify(text: string): string {
  return plain(text)
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
}

export function headings(body: string): Heading[] {
  const seen = new Map<string, number>()
  const found: Heading[] = []
  let fenced = false
  body.split('\n').forEach((line, at) => {
    if (/^\s*(```|~~~)/.test(line)) fenced = !fenced
    const match = fenced ? null : /^(##|###)\s+(.+?)\s*#*\s*$/.exec(line)
    if (!match) return
    const text = plain(match[2])
    const base = slugify(text)
    const count = seen.get(base) ?? 0
    seen.set(base, count + 1)
    found.push({ depth: match[1].length as 2 | 3, text, id: count ? `${base}-${count}` : base, line: at + 1 })
  })
  return found
}
