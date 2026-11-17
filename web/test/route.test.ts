import { describe, expect, test } from 'bun:test'
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { branded, fillBrand, findPage, pagesFrom, parsePage } from '../src/docs/pages'
import { internalTarget, parseRoute, pathOf } from '../src/docs/route'

describe('parseRoute', () => {
  test('the address gives the language, the page, the section and the search', () => {
    expect(parseRoute('/docs/en/hooks', '?q=events', '#the-events')).toEqual({ lang: 'en', slug: 'hooks', hash: 'the-events', query: 'events' })
    expect(parseRoute('/docs/hooks', '', '')).toEqual({ lang: null, slug: 'hooks', hash: '', query: '' })
    expect(parseRoute('/docs/pt', '', '')).toEqual({ lang: 'pt', slug: null, hash: '', query: '' })
    expect(parseRoute('/docs/', '', '')).toEqual({ lang: null, slug: null, hash: '', query: '' })
    expect(parseRoute('/docs/pt/hooks/', '', '')).toEqual({ lang: 'pt', slug: 'hooks', hash: '', query: '' })
  })

  test('a section written with an accent or a space is decoded', () => {
    expect(parseRoute('/docs/pt/hooks', '', '#configura%C3%A7%C3%A3o').hash).toBe('configuração')
    expect(parseRoute('/docs/en/hooks', '?q=model%20roles&x=1', '').query).toBe('model roles')
  })

  test('a section that is not valid percent-encoding is kept as written instead of breaking the page', () => {
    expect(parseRoute('/docs/en/budget-and-cost', '', '#100%').hash).toBe('100%')
    expect(parseRoute('/docs/en/hooks', '', '#%E0%A4%A').hash).toBe('%E0%A4%A')
  })
})

describe('pathOf', () => {
  test('a page has one address per language, with the section after it', () => {
    expect(pathOf('pt', 'hooks')).toBe('/docs/pt/hooks')
    expect(pathOf('en', 'mcp', 'oauth')).toBe('/docs/en/mcp#oauth')
    const there = pathOf('pt', 'settings', 'configuração')
    expect(parseRoute(there.split('#')[0], '', `#${there.split('#')[1]}`)).toEqual({ lang: 'pt', slug: 'settings', hash: 'configuração', query: '' })
  })
})

describe('internalTarget', () => {
  test('links to a page, a section or both stay inside the docs', () => {
    expect(internalTarget('hooks')).toEqual({ slug: 'hooks', id: '' })
    expect(internalTarget('./hooks')).toEqual({ slug: 'hooks', id: '' })
    expect(internalTarget('./hooks#events')).toEqual({ slug: 'hooks', id: 'events' })
    expect(internalTarget('#events')).toEqual({ slug: '', id: 'events' })
    expect(internalTarget('/docs/mcp#oauth')).toEqual({ slug: 'mcp', id: 'oauth' })
    expect(internalTarget('/docs/pt/mcp')).toEqual({ slug: 'mcp', id: '' })
    expect(internalTarget('/docs/en/mcp#oauth')).toEqual({ slug: 'mcp', id: 'oauth' })
  })

  test('other sites, mail, files and paths outside the docs leave it', () => {
    for (const href of ['https://example.com/a', 'http://localhost:3000/docs/en/mcp', 'mailto:someone@example.com', '/api/report', '../README.md', 'Hooks', '']) {
      expect(internalTarget(href)).toBeNull()
    }
  })

  test('a link to a language, not a page, is not taken for a page called en or pt', () => {
    expect(internalTarget('/docs/pt')).toBeNull()
    expect(internalTarget('/docs/en/')).toBeNull()
    expect(internalTarget('/docs/')).toBeNull()
  })
})

describe('placeholders', () => {
  const brand = { NAME: 'Acme', COMMAND: 'acme', DIR: '.acme', ENV: 'ACME', INSTRUCTIONS: 'ACME.md', LOCAL_INSTRUCTIONS: 'ACME.local.md', PLUGIN_DIR: '~/.acme/plugins' }

  test('every placeholder the docs use is filled, each time it appears', () => {
    const text = 'Run `{{COMMAND}}`; {{NAME}} reads {{DIR}}/, {{INSTRUCTIONS}}, {{LOCAL_INSTRUCTIONS}}, {{ENV}}_LOG and {{PLUGIN_DIR}}. {{NAME}}!'
    expect(fillBrand(text, brand)).toBe('Run `acme`; Acme reads .acme/, ACME.md, ACME.local.md, ACME_LOG and ~/.acme/plugins. Acme!')
  })

  test('a name the terminal did not give, and braces that are not a placeholder, stay as written', () => {
    expect(fillBrand('{{OTHER}} {{name}} {{ NAME }} {NAME} {{NAME}', brand)).toBe('{{OTHER}} {{name}} {{ NAME }} {NAME} {{NAME}')
    expect(fillBrand('{{NAME}}', {})).toBe('{{NAME}}')
  })

  test('a name that every object has is not a placeholder value', () => {
    expect(fillBrand('{{CONSTRUCTOR}} {{TO_STRING}} {{__PROTO__}}', {})).toBe('{{CONSTRUCTOR}} {{TO_STRING}} {{__PROTO__}}')
    expect(fillBrand('{{constructor}}', {})).toBe('{{constructor}}')
  })

  test('a value is written as it is, even when it holds a placeholder or a replacement pattern', () => {
    expect(fillBrand('{{NAME}}', { NAME: '$& {{DIR}} $1', DIR: 'x' })).toBe('$& {{DIR}} $1')
  })

  test('a page is filled in its title, its description and its body, and the original is left alone', () => {
    const page = parsePage('en', 'about', '---\ntitle: About {{NAME}}\ndescription: "What {{COMMAND}} does"\n---\nRun {{COMMAND}} in {{DIR}}.\n')
    const filled = branded(page, brand)
    expect([filled.title, filled.description, filled.body]).toEqual(['About Acme', 'What acme does', 'Run acme in .acme.\n'])
    expect(page.title).toBe('About {{NAME}}')
    expect([filled.lang, filled.slug]).toEqual(['en', 'about'])
  })

  test('the written docs use only the placeholders the terminal gives a value for', () => {
    const served = new Set(['NAME', 'COMMAND', 'DIR', 'INSTRUCTIONS', 'LOCAL_INSTRUCTIONS', 'PLUGIN_DIR', 'ENV', 'COMPANY'])
    const root = path.join(import.meta.dir, '..', 'docs')
    const files: Record<string, string> = {}
    for (const lang of ['en', 'pt']) {
      for (const name of readdirSync(path.join(root, lang))) files[`../../docs/${lang}/${name}`] = readFileSync(path.join(root, lang, name), 'utf8')
    }
    const unfilled: string[] = []
    for (const [key, page] of pagesFrom(files)) {
      for (const [whole, name] of `${page.title}\n${page.description}\n${page.body}`.matchAll(/\{\{\s*([A-Za-z_]+)\s*\}\}/g)) {
        if (whole !== `{{${name}}}` || !served.has(name)) unfilled.push(`${key}: ${whole}`)
      }
    }
    expect(unfilled).toEqual([])
    expect(findPage(pagesFrom(files), 'pt', 'hooks')?.lang).toBe('pt')
  })
})
