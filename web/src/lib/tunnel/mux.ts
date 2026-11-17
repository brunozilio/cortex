import { fromBase64, toBase64 } from './bytes'
import type { Answer, AnswerChannel, Ask, Payload } from './messages'
import { EventParser } from './sse'

export const SOCKET_OPENED = 101
const RECONNECT_MS = 2_000

interface Pending {
  head?: (status: number, contentType: string | undefined) => void
  data?: (data: Payload) => void
  end?: (error: string | undefined) => void
}

function bytesOf(payload: Payload): Uint8Array {
  return payload.kind === 'text' ? new TextEncoder().encode(payload.value) : fromBase64(payload.value)
}

function pathOf(url: string | URL): string {
  const parsed = new URL(String(url), 'http://page.invalid')
  return parsed.pathname + parsed.search
}

export class Tunnel {
  private next = 1
  private readonly pending = new Map<number, Pending>()

  constructor(private readonly channel: AnswerChannel) {
    channel.onAnswer = (answer) => this.answer(answer)
    channel.onDown = (reason) => {
      for (const [id, pending] of [...this.pending]) {
        this.pending.delete(id)
        pending.end?.(reason)
      }
    }
  }

  private answer(answer: Answer) {
    const pending = this.pending.get(answer.id)
    if (!pending) return
    if (answer.type === 'head') pending.head?.(answer.status, answer.content_type)
    if (answer.type === 'data') pending.data?.(answer.data)
    if (answer.type === 'end') {
      this.pending.delete(answer.id)
      pending.end?.(answer.error)
    }
  }

  private open(ask: (id: number) => Ask, pending: Pending): number {
    const id = this.next++
    this.pending.set(id, pending)
    this.channel.send(ask(id))
    return id
  }

  fetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
    const url = input instanceof Request ? input.url : input
    const method = (init.method ?? (input instanceof Request ? input.method : 'GET')).toUpperCase()
    const headers = new Headers(init.headers)
    const body = typeof init.body === 'string' ? init.body : undefined
    return new Promise((resolve, reject) => {
      let status = 0
      let contentType: string | undefined
      const chunks: Uint8Array[] = []
      this.open((id) => ({ type: 'fetch', id, method, path: pathOf(url), body, content_type: headers.get('content-type') ?? undefined }), {
        head: (given, kind) => {
          status = given
          contentType = kind
        },
        data: (data) => chunks.push(bytesOf(data)),
        end: (error) => {
          if (status === 0) return reject(new TypeError(error ?? 'the machine did not answer'))
          const headers = contentType ? { 'content-type': contentType } : undefined
          const nothing = status === 204 || status === 304
          resolve(new Response(nothing ? null : new Blob(chunks as BlobPart[]), { status, headers }))
        },
      })
    })
  }

  events(url: string | URL): TunnelEventSource {
    return new TunnelEventSource(this, pathOf(url))
  }

  socket(url: string | URL): TunnelSocket {
    return new TunnelSocket(this, pathOf(url))
  }

  openEvents(path: string, pending: Pending): number {
    return this.open((id) => ({ type: 'events', id, path }), pending)
  }

  openSocket(path: string, pending: Pending): number {
    return this.open((id) => ({ type: 'socket', id, path }), pending)
  }

  send(id: number, data: Payload) {
    this.channel.send({ type: 'send', id, data })
  }

  close(id: number) {
    if (this.pending.delete(id)) this.channel.send({ type: 'close', id })
  }
}

export class TunnelEventSource extends EventTarget {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSED = 2
  readonly CONNECTING = 0
  readonly OPEN = 1
  readonly CLOSED = 2
  readyState = 0
  onopen: ((event: Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  private id = 0
  private retry: ReturnType<typeof setTimeout> | undefined

  constructor(
    private readonly tunnel: Tunnel,
    readonly url: string,
  ) {
    super()
    this.connect()
  }

  private connect() {
    const parser = new EventParser()
    const decoder = new TextDecoder()
    this.readyState = this.CONNECTING
    this.id = this.tunnel.openEvents(this.url, {
      head: (status) => {
        if (status !== 200) return
        this.readyState = this.OPEN
        this.fire(new Event('open'))
      },
      data: (data) => {
        for (const { event, data: text } of parser.push(decoder.decode(bytesOf(data), { stream: true }))) {
          this.fire(new MessageEvent(event, { data: text }))
        }
      },
      end: () => {
        if (this.readyState === this.CLOSED) return
        this.readyState = this.CONNECTING
        this.fire(new Event('error'))
        this.retry = setTimeout(() => this.connect(), RECONNECT_MS)
      },
    })
  }

  private fire(event: Event) {
    if (event.type === 'open') this.onopen?.(event)
    if (event.type === 'error') this.onerror?.(event)
    if (event.type === 'message') this.onmessage?.(event as MessageEvent<string>)
    this.dispatchEvent(event)
  }

  close() {
    this.readyState = this.CLOSED
    clearTimeout(this.retry)
    this.tunnel.close(this.id)
  }
}

export class TunnelSocket extends EventTarget {
  static readonly CONNECTING = 0
  static readonly OPEN = 1
  static readonly CLOSING = 2
  static readonly CLOSED = 3
  readonly CONNECTING = 0
  readonly OPEN = 1
  readonly CLOSING = 2
  readonly CLOSED = 3
  readyState = 0
  binaryType: BinaryType = 'blob'
  onopen: ((event: Event) => void) | null = null
  onclose: ((event: CloseEvent | Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  private readonly id: number

  constructor(
    private readonly tunnel: Tunnel,
    readonly url: string,
  ) {
    super()
    this.id = tunnel.openSocket(url, {
      head: (status) => {
        if (status !== SOCKET_OPENED) return
        this.readyState = this.OPEN
        this.fire(new Event('open'))
      },
      data: (data) => {
        if (this.readyState !== this.OPEN) return
        const bytes = bytesOf(data)
        const message =
          data.kind === 'text' ? data.value : this.binaryType === 'arraybuffer' ? bytes.slice().buffer : new Blob([bytes as BlobPart])
        this.fire(new MessageEvent('message', { data: message }))
      },
      end: (error) => {
        if (error && this.readyState !== this.OPEN) this.fire(new Event('error'))
        this.ended()
      },
    })
  }

  private ended() {
    if (this.readyState === this.CLOSED) return
    this.readyState = this.CLOSED
    this.fire(new Event('close'))
  }

  private fire(event: Event) {
    if (event.type === 'open') this.onopen?.(event)
    if (event.type === 'close') this.onclose?.(event)
    if (event.type === 'error') this.onerror?.(event)
    if (event.type === 'message') this.onmessage?.(event as MessageEvent)
    this.dispatchEvent(event)
  }

  send(data: string | ArrayBuffer | ArrayBufferView) {
    if (this.readyState !== this.OPEN) return
    if (typeof data === 'string') return this.tunnel.send(this.id, { kind: 'text', value: data })
    const bytes = data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
    this.tunnel.send(this.id, { kind: 'bytes', value: toBase64(bytes) })
  }

  close() {
    this.tunnel.close(this.id)
    this.ended()
  }
}
