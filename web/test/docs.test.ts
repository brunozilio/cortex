import { describe, expect, test } from 'bun:test'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { headings, slugify } from '../src/docs/headings'
import { branded, fillBrand, findPage, pagesFrom, parsePage } from '../src/docs/pages'
import { internalTarget, parseRoute, pathOf } from '../src/docs/route'
import { search, sections } from '../src/docs/search'
import { neighbours, ORDER } from '../src/docs/sidebar'

const BODY = [
  '## Getting started',
  'Run `{{COMMAND}}` once.',
  '```bash',
  '## not a heading',
  '```',
  '### Options',
  'The `permissions.allow` key lists rules.',
  '## Options',
  'Again, with ação.',
].join('\n')

describe('headings', () => {
  test('ids are slugs, unique, and skip code', () => {
    expect(headings(BODY).map((heading) => [heading.depth, heading.id, heading.line])).toEqual([
      [2, 'getting-started', 1],
      [3, 'options', 6],
      [2, 'options-1', 8],
    ])
  })

  test('slugs drop accents, code marks and links', () => {
    expect(slugify('Configuração `settings.json` [aqui](./x)')).toBe('configuracao-settingsjson-aqui')
  })
})

describe('pages', () => {
  test('frontmatter gives the title and description', () => {
    const page = parsePage('pt', 'settings', '---\ntitle: Configurações\ndescription: "Tudo"\n---\n\n## A\n')
    expect([page.title, page.description, page.body]).toEqual(['Configurações', 'Tudo', '\n## A\n'])
  })

  test('placeholders take the brand and unknown ones stay', () => {
    expect(fillBrand('{{NAME}} reads {{DIR}}/{{OTHER}}', { NAME: 'Acme', DIR: '.acme' })).toBe('Acme reads .acme/{{OTHER}}')
    const page = branded(parsePage('en', 'a', '---\ntitle: About {{NAME}}\n---\n{{NAME}}'), { NAME: 'Acme' })
    expect([page.title, page.body]).toEqual(['About Acme', 'Acme'])
  })

  test('a missing translation falls back to English', () => {
    const pages = pagesFrom({ '../../docs/en/a.md': '---\ntitle: A\n---\n', '../../docs/pt/b.md': '---\ntitle: B\n---\n' })
    expect(findPage(pages, 'pt', 'a')?.lang).toBe('en')
    expect(findPage(pages, 'pt', 'b')?.lang).toBe('pt')
    expect(findPage(pages, 'pt', 'c')).toBeUndefined()
  })
})

describe('routes', () => {
  test('paths carry the language, the page and the section', () => {
    expect(parseRoute('/docs/pt/hooks', '', '#eventos')).toEqual({ lang: 'pt', slug: 'hooks', hash: 'eventos', query: '' })
    expect(parseRoute('/docs', '?q=model%20roles', '')).toEqual({ lang: null, slug: null, hash: '', query: 'model roles' })
    expect(pathOf('en', 'mcp', 'oauth')).toBe('/docs/en/mcp#oauth')
  })

  test('links between pages stay in the site and others leave it', () => {
    expect(internalTarget('./hooks#events')).toEqual({ slug: 'hooks', id: 'events' })
    expect(internalTarget('#events')).toEqual({ slug: '', id: 'events' })
    expect(internalTarget('/docs/pt/mcp')).toEqual({ slug: 'mcp', id: '' })
    expect(internalTarget('https://example.com/a')).toBeNull()
  })
})

describe('search', () => {
  const page = parsePage('en', 'settings', `---\ntitle: Settings\ndescription: Every key\n---\n${BODY}`)
  const index = sections(page)

  test('every section is indexed under its heading', () => {
    expect(index.map((section) => section.id)).toEqual(['', 'getting-started', 'options', 'options-1'])
  })

  test('a heading match ranks above a text match and accents are ignored', () => {
    const hits = search(index, 'options')
    expect(hits[0].heading).toBe('Options')
    expect(search(index, 'acao')[0].id).toBe('options-1')
    expect(search(index, 'permissions.allow')[0].id).toBe('options')
    expect(search(index, 'nothing-like-this')).toEqual([])
  })
})

describe('the written docs', () => {
  const root = path.join(import.meta.dir, '..', 'docs')
  const read = (lang: string) => new Set(readdirSync(path.join(root, lang)).map((name) => name.replace(/\.md$/, '')))

  test('every page in the sidebar exists in both languages, and every page is in the sidebar', () => {
    for (const lang of ['en', 'pt']) {
      const written = read(lang)
      expect(ORDER.filter((slug) => !written.has(slug))).toEqual([])
      expect([...written].filter((slug) => !ORDER.includes(slug))).toEqual([])
    }
  })

  test('every link to another page points at a page that exists', () => {
    const broken: string[] = []
    for (const lang of ['en', 'pt']) {
      for (const slug of read(lang)) {
        const text = readFileSync(path.join(root, lang, `${slug}.md`), 'utf8')
        const own = new Set(headings(text).map((heading) => heading.id))
        for (const [, href] of text.matchAll(/\]\(([^)\s]+)\)/g)) {
          const target = internalTarget(href)
          if (!target) continue
          const page = target.slug || slug
          if (!ORDER.includes(page)) broken.push(`${lang}/${slug}: ${href}`)
          else if (target.id) {
            const other = target.slug ? readFileSync(path.join(root, lang, `${page}.md`), 'utf8') : text
            const ids = target.slug ? new Set(headings(other).map((heading) => heading.id)) : own
            if (!ids.has(target.id)) broken.push(`${lang}/${slug}: ${href}`)
          }
        }
      }
    }
    expect(broken).toEqual([])
  })

  test('the pages read in order with neighbours', () => {
    expect(neighbours(ORDER[0]).previous).toBeUndefined()
    expect(neighbours(ORDER[1]).previous).toBe(ORDER[0])
  })
})
