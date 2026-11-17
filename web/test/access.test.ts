import { describe, expect, test } from 'bun:test'
import { ACCESS_QUERY, ENTER_PATH, ENTRY_HEADER, authorized, enter, entryCode, installAccess, withToken, type KeptToken } from '../src/lib/access'

const CODE = 'ab'.repeat(32)
const place = { pathname: '/console', search: '?tab=1', hash: `#enter=${CODE}`, host: '127.0.0.1:4000', protocol: 'http:' }

function memory(start: string | null = null): KeptToken & { value: string | null } {
  return {
    value: start,
    get() {
      return this.value
    },
    set(token) {
      this.value = token
    },
  }
}

describe('the console page enters with a one-time code and then carries its token itself', () => {
  test('the code is read from the fragment only when it looks like one', () => {
    expect(entryCode(`#enter=${CODE}`)).toBe(CODE)
    expect(entryCode('#enter=<script>')).toBeNull()
    expect(entryCode('')).toBeNull()
  })

  test('entering takes the code out of the address, trades it once and keeps the token', async () => {
    const replaced: string[] = []
    const sent: { url: string; init?: RequestInit }[] = []
    const kept = memory()
    const fetcher = (async (url: string, init?: RequestInit) => {
      sent.push({ url, init })
      return new Response(JSON.stringify({ token: 'tok' }), { status: 200 })
    }) as unknown as typeof fetch
    expect(await enter(place, { replaceState: (_d, _u, url) => replaced.push(url) }, kept, fetcher)).toBe(true)
    expect(replaced).toEqual(['/console?tab=1'])
    expect(sent[0].url).toBe(ENTER_PATH)
    expect(sent[0].init?.method).toBe('POST')
    expect((sent[0].init?.headers as Record<string, string>)[ENTRY_HEADER]).toBe(CODE)
    expect(kept.value).toBe('tok')
  })

  test('a refused code keeps no token', async () => {
    const kept = memory()
    const fetcher = (async () => new Response('{}', { status: 401 })) as unknown as typeof fetch
    expect(await enter(place, { replaceState: () => undefined }, kept, fetcher)).toBe(false)
    expect(kept.value).toBeNull()
  })

  test('only this server s API gets the token, as a header or in the query', () => {
    const [, init] = authorized('/api/report', undefined, place, 'tok')
    expect(new Headers(init?.headers).get('authorization')).toBe('Bearer tok')
    const [, elsewhere] = authorized('http://127.0.0.1:5173/api/report', undefined, place, 'tok')
    expect(elsewhere).toBeUndefined()
    const [, page] = authorized('/assets/app.js', undefined, place, 'tok')
    expect(page).toBeUndefined()
    expect(withToken('/api/events', place, 'tok')).toBe(`http://127.0.0.1:4000/api/events?${ACCESS_QUERY}=tok`)
    expect(withToken('ws://127.0.0.1:4000/api/terminal?cols=80', place, 'tok')).toBe(`ws://127.0.0.1:4000/api/terminal?cols=80&${ACCESS_QUERY}=tok`)
    expect(withToken('ws://127.0.0.1:4001/api/terminal', place, 'tok')).toBe('ws://127.0.0.1:4001/api/terminal')
    expect(withToken('/api/events', place, null)).toBe('/api/events')
  })

  test('installing wraps fetch, event streams and sockets with the kept token', async () => {
    const seen: string[] = []
    const scope: Record<string, unknown> = {
      fetch: async (input: RequestInfo | URL, init?: RequestInit) => {
        seen.push(`${String(input)} ${new Headers(init?.headers).get('authorization')}`)
        return new Response('')
      },
      EventSource: class {
        constructor(url: string) {
          seen.push(`events ${url}`)
        }
      },
      WebSocket: class {
        constructor(url: string) {
          seen.push(`socket ${url}`)
        }
      },
    }
    installAccess(scope, place, memory('tok'))
    await (scope.fetch as typeof fetch)('/api/sessions')
    new (scope.EventSource as new (url: string) => unknown)('/api/events')
    new (scope.WebSocket as new (url: string) => unknown)('ws://127.0.0.1:4000/api/terminal')
    expect(seen).toEqual([
      '/api/sessions Bearer tok',
      `events http://127.0.0.1:4000/api/events?${ACCESS_QUERY}=tok`,
      `socket ws://127.0.0.1:4000/api/terminal?${ACCESS_QUERY}=tok`,
    ])
  })
})
