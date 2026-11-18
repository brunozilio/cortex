import { describe, expect, test } from 'bun:test'
import { CHAT_WORDS, ago, askView, bubbles, chatApi, folderName, initials, pathOf, placeOf, type Entry } from '../src/lib/chats'

const words = CHAT_WORDS.en

describe('bubbles', () => {
  test('the owner speaks on one side, the agent on the other, and its steps fold into one line', () => {
    const entries: Entry[] = [
      { kind: 'user', data: 'run the tests' },
      { kind: 'thinking', data: { text: 'hmm', done: true } },
      { kind: 'tool', data: { id: 't1', name: 'Bash', summary: 'cargo test', state: 'done', output: null } },
      { kind: 'tool', data: { id: 't2', name: 'Read', summary: 'src/lib.rs', state: 'failed', output: null } },
      { kind: 'agent', data: { index: 1, kind: 'explore', summary: 'look around', done: null, steps: [] } },
      { kind: 'assistant', data: 'All pass.' },
      { kind: 'assistant', data: '   ' },
      { kind: 'finished', data: { duration_ms: 1, cost_usd: null } },
      { kind: 'error', data: 'rate limited' },
    ]
    expect(bubbles(entries, words)).toEqual([
      { kind: 'mine', text: 'run the tests' },
      {
        kind: 'steps',
        steps: [
          { name: 'Bash', summary: 'cargo test', state: 'done' },
          { name: 'Read', summary: 'src/lib.rs', state: 'failed' },
          { name: 'agent explore', summary: 'look around', state: 'running' },
        ],
      },
      { kind: 'theirs', text: 'All pass.' },
      { kind: 'note', text: 'rate limited', tone: 'bad' },
    ])
  })

  test('a plan is the agent speaking, under its label', () => {
    expect(bubbles([{ kind: 'plan', data: { plan: '1. fix', outcome: 'Pending' } }], words)).toEqual([{ kind: 'theirs', text: '1. fix', label: 'Plan' }])
  })
})

describe('askView', () => {
  test('a permission offers allow, always when it can be remembered, and deny', () => {
    const ask = { id: 'a1', kind: { Permission: { tool: 'Bash', subject: 'rm -rf target', rememberable: true } } }
    const view = askView(ask, words)
    expect(view.title).toBe('Allow Bash?')
    expect(view.detail).toBe('rm -rf target')
    expect(view.choices.map((choice) => choice.decision)).toEqual(['allow', 'always', 'deny'])
    const once = askView({ id: 'a2', kind: { Permission: { tool: 'Bash', subject: 'x', rememberable: false } } }, words)
    expect(once.choices.map((choice) => choice.decision)).toEqual(['allow', 'deny'])
  })

  test('a plan is approved or sent back, and questions can only be skipped from the phone', () => {
    expect(askView({ id: 'p', kind: { Plan: { plan: 'steps' } } }, words).choices.map((choice) => choice.decision)).toEqual(['approve', 'deny'])
    expect(askView({ id: 'q', kind: { Questions: [] } }, words).choices.map((choice) => choice.decision)).toEqual(['deny'])
  })
})

describe('places', () => {
  test('the chat list, a new chat and one chat each have an address', () => {
    expect(placeOf('/chat')).toEqual({ screen: 'list' })
    expect(placeOf('/chat/new')).toEqual({ screen: 'new' })
    expect(placeOf('/chat/abc-1_2')).toEqual({ screen: 'chat', session: 'abc-1_2' })
    expect(placeOf('/chat/../etc')).toBeNull()
    expect(placeOf('/trace')).toBeNull()
    expect(pathOf({ screen: 'chat', session: 's1' })).toBe('/chat/s1')
  })
})

describe('labels', () => {
  test('times read as how long ago, folders by their last name, avatars by two letters', () => {
    const now = 10 * 24 * 3_600_000
    expect(ago(now - 10_000, now, words)).toBe('now')
    expect(ago(now - 5 * 60_000, now, words)).toBe('5 min')
    expect(ago(now - 3 * 3_600_000, now, words)).toBe('3 h')
    expect(ago(now - 2 * 24 * 3_600_000, now, words)).toBe('2 d')
    expect(folderName('/Users/me/work/lite/')).toBe('lite')
    expect(initials('Fix the build')).toBe('FT')
    expect(initials('lite')).toBe('LI')
  })
})

describe('chatApi', () => {
  test('it posts messages and answers as JSON and reads the server error message', async () => {
    const calls: [string, RequestInit | undefined][] = []
    const fetcher = (async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push([String(input), init])
      if (String(input).endsWith('/messages')) {
        return new Response(JSON.stringify({ error: { code: 'busy', message: 'still answering' } }), { status: 409 })
      }
      return new Response(JSON.stringify({ session: 's9' }))
    }) as typeof fetch
    const api = chatApi(fetcher)

    expect(await api.start('hello')).toBe('s9')
    await api.answer('s9', 'a1', 'allow')
    await expect(api.send('s9', 'again')).rejects.toThrow('still answering')
    expect(calls.map(([url, init]) => [url, init?.body])).toEqual([
      ['/api/chats', '{"text":"hello"}'],
      ['/api/chats/s9/answer', '{"ask":"a1","decision":"allow"}'],
      ['/api/chats/s9/messages', '{"text":"again"}'],
    ])
  })
})
