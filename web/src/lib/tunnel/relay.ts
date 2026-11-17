import type { Answer, AnswerChannel, Ask } from './messages'
import type { Initiator, Transport } from './noise'

export interface SocketLike {
  binaryType: BinaryType
  readyState: number
  onopen: ((event: Event) => void) | null
  onmessage: ((event: MessageEvent) => void) | null
  onclose: ((event: CloseEvent | Event) => void) | null
  onerror: ((event: Event) => void) | null
  send(data: ArrayBufferView | ArrayBuffer | string): void
  close(): void
}

export interface Greeting {
  initiator: Initiator
  payload: Uint8Array
  confirms?: boolean
}

export interface RelayOptions {
  socketUrl: string
  greet: () => Greeting
  open?: (url: string) => SocketLike
  onUp?: () => void
}

const OPEN = 1
export const KEEPALIVE_MS = 25_000
export const CONFIRMATION: Ask = { type: 'close', id: 0 }

export class RelayChannel implements AnswerChannel {
  onAnswer: (answer: Answer) => void = () => undefined
  onDown: (reason: string) => void = () => undefined
  private socket: SocketLike | null = null
  private transport: Transport | null = null
  private queued: Ask[] = []
  private connecting: Promise<void> | null = null

  constructor(private readonly options: RelayOptions) {}

  connect(): Promise<void> {
    this.connecting ??= this.dial().finally(() => {
      this.connecting = null
    })
    return this.connecting
  }

  private dial(): Promise<void> {
    const { initiator, payload, confirms } = this.options.greet()
    const open = this.options.open ?? ((url: string) => new WebSocket(url) as SocketLike)
    const socket = open(this.options.socketUrl)
    socket.binaryType = 'arraybuffer'
    this.socket = socket
    return new Promise((resolve, reject) => {
      let greeted = false
      socket.onopen = () => socket.send(initiator.hello(payload))
      const keepalive = setInterval(() => {
        if (socket.readyState === OPEN) socket.send('ping')
      }, KEEPALIVE_MS)
      socket.onmessage = (event: MessageEvent) => {
        if (typeof event.data === 'string') return
        const bytes = new Uint8Array(event.data as ArrayBuffer)
        if (!greeted) {
          greeted = true
          try {
            this.transport = initiator.finish(bytes)
          } catch (error) {
            socket.close()
            return reject(error)
          }
          if (confirms) this.queued.unshift(CONFIRMATION)
          this.options.onUp?.()
          resolve()
          return this.flush()
        }
        this.received(bytes)
      }
      socket.onclose = () => {
        clearInterval(keepalive)
        const reason = greeted ? 'the connection to the machine closed' : 'the machine did not let this device in'
        if (this.socket === socket) {
          this.socket = null
          this.transport = null
        }
        if (!greeted) reject(new Error(reason))
        this.onDown(reason)
      }
      socket.onerror = () => undefined
    })
  }

  private received(sealed: Uint8Array) {
    try {
      const plain = this.transport?.open(sealed)
      if (plain) this.onAnswer(JSON.parse(new TextDecoder().decode(plain)) as Answer)
    } catch {
      this.socket?.close()
    }
  }

  private flush() {
    const socket = this.socket
    const transport = this.transport
    if (!socket || !transport || socket.readyState !== OPEN) return
    for (const ask of this.queued.splice(0)) {
      for (const piece of transport.seal(new TextEncoder().encode(JSON.stringify(ask)))) socket.send(piece)
    }
  }

  send(ask: Ask) {
    this.queued.push(ask)
    if (this.transport) return this.flush()
    this.connect().catch(() => {
      const dropped = this.queued.splice(0)
      if (dropped.length > 0) this.onDown('the machine is not reachable')
    })
  }

  close() {
    this.socket?.close()
  }
}
