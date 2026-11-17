import { afterEach, describe, expect, test } from 'bun:test'
import { loadSessions, served, watchSession, type SessionCard } from '../src/lib/sessions'
import { answers, fixture } from './make'

const real = globalThis.fetch

afterEach(() => {
  globalThis.fetch = real
})

function recording(status: number): { calls: { url: string; init: RequestInit | undefined }[] } {
  const calls: { url: string; init: RequestInit | undefined }[] = []
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push({ url: String(input), init })
    return new Response('{}', { status })
  }) as unknown as typeof fetch
  return { calls }
}

describe('watchSession', () => {
  test('asks the console, in JSON, to follow that session', async () => {
    const { calls } = recording(202)

    await watchSession('abc-123')

    expect(calls.length).toBe(1)
    expect(calls[0].url).toBe('/api/watch')
    expect(calls[0].init?.method).toBe('POST')
    expect(new Headers(calls[0].init?.headers).get('content-type')).toBe('application/json')
    expect(JSON.parse(String(calls[0].init?.body))).toEqual({ session: 'abc-123' })
  })

  test('a refusal is an error that carries the status', async () => {
    recording(400)
    await expect(watchSession('--help')).rejects.toThrow('400')
  })

  test('every path the page asks for is under the API', () => {
    expect(served('/sessions')).toBe('/api/sessions')
  })
})

describe('loadSessions', () => {
  const cards = fixture('sessions.json') as SessionCard[]

  test('the cards the terminal listed are returned as they are', async () => {
    const { fetcher, asked } = answers({ '/api/sessions': { body: cards } })
    expect(await loadSessions(fetcher)).toEqual(cards)
    expect(asked).toEqual(['/api/sessions'])
  })

  test('no answer, a refusal or a list this page cannot read leaves the cards as they were', async () => {
    expect(await loadSessions(answers({ '/api/sessions': new TypeError('fetch failed') }).fetcher)).toBeNull()
    expect(await loadSessions(answers({ '/api/sessions': { status: 401, body: {} } }).fetcher)).toBeNull()
    expect(await loadSessions(answers({ '/api/sessions': { body: 'not json' } }).fetcher)).toBeNull()
    expect(await loadSessions(answers({ '/api/sessions': { body: [{ ...cards[0], status: 'sleeping' }] } }).fetcher)).toBeNull()
    expect(await loadSessions(answers({ '/api/sessions': { body: { error: {} } } }).fetcher)).toBeNull()
  })
})
