import { describe, expect, test } from 'bun:test'
import { toBase64 } from '../src/lib/tunnel/bytes'
import type { Answer, AnswerChannel, Ask } from '../src/lib/tunnel/messages'
import { Tunnel } from '../src/lib/tunnel/mux'
import { EventParser } from '../src/lib/tunnel/sse'

class FakeChannel implements AnswerChannel {
  asked: Ask[] = []
  onAnswer: (answer: Answer) => void = () => undefined
  onDown: (reason: string) => void = () => undefined
  send(ask: Ask) {
    this.asked.push(ask)
  }
  answer(...answers: Answer[]) {
    for (const answer of answers) this.onAnswer(answer)
  }
}

const bytes = (text: string) => ({ kind: 'bytes' as const, value: toBase64(new TextEncoder().encode(text)) })

describe('the tunnel stands in for fetch, EventSource and WebSocket', () => {
  test('a fetch asks for the path alone and resolves with the machine status, type and body', async () => {
    const channel = new FakeChannel()
    const tunnel = new Tunnel(channel)
    const answered = tunnel.fetch('/api/sessions/watch?x=1', { method: 'post', headers: { 'Content-Type': 'application/json' }, body: '{"a":1}' })
    expect(channel.asked).toEqual([{ type: 'fetch', id: 1, method: 'POST', path: '/api/sessions/watch?x=1', body: '{"a":1}', content_type: 'application/json' }])
    channel.answer(
      { type: 'head', id: 1, status: 200, content_type: 'application/json' },
      { type: 'data', id: 1, data: bytes('{"ok":') },
      { type: 'data', id: 1, data: bytes('true}') },
      { type: 'end', id: 1 },
    )
    const response = await answered
    expect(response.status).toBe(200)
    expect(response.headers.get('content-type')).toBe('application/json')
    expect(await response.json()).toEqual({ ok: true })
  })

  test('a fetch with no answer from the machine fails like a network error', async () => {
    const channel = new FakeChannel()
    const tunnel = new Tunnel(channel)
    const answered = tunnel.fetch('http://relay.example/api/report')
    expect(channel.asked[0]).toMatchObject({ path: '/api/report', method: 'GET' })
    channel.onDown('the relay closed')
    await expect(answered).rejects.toThrow('the relay closed')
  })

  test('an event stream opens, delivers named and plain events across chunk borders, and closes', () => {
    const channel = new FakeChannel()
    const source = new Tunnel(channel).events('/api/events')
    const seen: string[] = []
    source.onopen = () => seen.push('open')
    source.onmessage = (event) => seen.push(`message:${event.data}`)
    source.addEventListener('report', () => seen.push('report'))
    channel.answer({ type: 'head', id: 1, status: 200, content_type: 'text/event-stream' }, { type: 'data', id: 1, data: bytes('event: report\ndata: x\n\ndata: {"a"') }, { type: 'data', id: 1, data: bytes(':1}\n\n') })
    expect(seen).toEqual(['open', 'report', 'message:{"a":1}'])
    expect(source.readyState).toBe(1)
    source.close()
    expect(channel.asked.at(-1)).toEqual({ type: 'close', id: 1 })
    expect(source.readyState).toBe(2)
  })

  test('a socket opens on 101, carries text and bytes both ways, and closes when the machine ends it', () => {
    const channel = new FakeChannel()
    const socket = new Tunnel(channel).socket('ws://relay.example/api/terminal?session=a&cols=80&rows=24')
    socket.binaryType = 'arraybuffer'
    const seen: unknown[] = []
    socket.onopen = () => seen.push('open')
    socket.onmessage = (event) => seen.push(typeof event.data === 'string' ? event.data : new TextDecoder().decode(event.data as ArrayBuffer))
    socket.onclose = () => seen.push('close')
    expect(channel.asked[0]).toEqual({ type: 'socket', id: 1, path: '/api/terminal?session=a&cols=80&rows=24' })
    socket.send('{"cols":100,"rows":30}')
    expect(channel.asked).toHaveLength(1)
    channel.answer({ type: 'head', id: 1, status: 101 }, { type: 'data', id: 1, data: bytes('$ ') }, { type: 'data', id: 1, data: { kind: 'text', value: '{"cols":80,"rows":24}' } })
    socket.send(new TextEncoder().encode('ls\r'))
    socket.send('{"cols":100,"rows":30}')
    expect(channel.asked.slice(1)).toEqual([
      { type: 'send', id: 1, data: { kind: 'bytes', value: toBase64(new TextEncoder().encode('ls\r')) } },
      { type: 'send', id: 1, data: { kind: 'text', value: '{"cols":100,"rows":30}' } },
    ])
    channel.answer({ type: 'end', id: 1 })
    expect(seen).toEqual(['open', '$ ', '{"cols":80,"rows":24}', 'close'])
    expect(socket.readyState).toBe(3)
  })

  test('a socket the console refuses reports an error and closes', () => {
    const channel = new FakeChannel()
    const socket = new Tunnel(channel).socket('/api/terminal')
    const seen: string[] = []
    socket.onerror = () => seen.push('error')
    socket.onclose = () => seen.push('close')
    channel.answer({ type: 'head', id: 1, status: 429 }, { type: 'end', id: 1, error: 'too many terminals' })
    expect(seen).toEqual(['error', 'close'])
  })
})

describe('server-sent events are read as the browser reads them', () => {
  test('fields, comments, multi-line data and every line ending', () => {
    const parser = new EventParser()
    expect(parser.push(': hello\r\nevent: resync\r\ndata: a\r\ndata: b\r\n\r\n')).toEqual([{ event: 'resync', data: 'a\nb' }])
    expect(parser.push('data:no-space\r\r')).toEqual([{ event: 'message', data: 'no-space' }])
    expect(parser.push(': x\n\n')).toEqual([])
  })
})
