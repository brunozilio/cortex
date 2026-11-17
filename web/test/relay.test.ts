import { describe, expect, test } from 'bun:test'
import transcript from './fixtures/noise.json'
import { fromHex, toHex } from '../src/lib/tunnel/bytes'
import type { Answer } from '../src/lib/tunnel/messages'
import { keyFrom } from '../src/lib/tunnel/noise'
import { pairedInitiator } from '../src/lib/tunnel/link'
import { RelayChannel, type SocketLike } from '../src/lib/tunnel/relay'

class FakeSocket implements SocketLike {
  binaryType: BinaryType = 'blob'
  readyState = 0
  sent: string[] = []
  onopen: ((event: Event) => void) | null = null
  onmessage: ((event: MessageEvent) => void) | null = null
  onclose: ((event: Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  constructor(readonly url: string) {}
  send(data: ArrayBufferView | ArrayBuffer) {
    const view = data instanceof ArrayBuffer ? new Uint8Array(data) : new Uint8Array(data.buffer, data.byteOffset, data.byteLength)
    this.sent.push(toHex(view))
  }
  close() {
    this.readyState = 3
    this.onclose?.(new Event('close'))
  }
  opened() {
    this.readyState = 1
    this.onopen?.(new Event('open'))
  }
  receive(hex: string) {
    this.onmessage?.(new MessageEvent('message', { data: fromHex(hex).slice().buffer }))
  }
}

const t = transcript.paired

function channel() {
  const sockets: FakeSocket[] = []
  const relay = new RelayChannel({
    socketUrl: `wss://relay.example/phone?machine=${t.machine_id}`,
    greet: () => ({
      initiator: pairedInitiator(keyFrom(fromHex(t.device_private)), fromHex(t.machine_public), fromHex(t.ephemeral_private)),
      payload: new Uint8Array(0),
    }),
    open: (url) => {
      const socket = new FakeSocket(url)
      sockets.push(socket)
      return socket
    },
  })
  return { relay, sockets }
}

describe('the relay channel', () => {
  test('greets the machine, sends what was asked meanwhile, sealed, and hands over what the machine answers', async () => {
    const { relay, sockets } = channel()
    const answers: Answer[] = []
    relay.onAnswer = (answer) => answers.push(answer)
    relay.send(JSON.parse(new TextDecoder().decode(fromHex(t.transport.phone_to_machine.plain))))
    const socket = sockets[0]
    expect(socket.url).toBe(`wss://relay.example/phone?machine=${t.machine_id}`)
    socket.opened()
    expect(socket.sent).toEqual([t.hello])
    socket.receive(t.reply)
    await Promise.resolve()
    expect(socket.sent.slice(1)).toEqual(t.transport.phone_to_machine.sealed)
    for (const piece of t.transport.machine_to_phone.sealed) socket.receive(piece)
    expect(answers).toEqual([JSON.parse(new TextDecoder().decode(fromHex(t.transport.machine_to_phone.plain)))])
  })

  test('says the device was not let in when the machine closes before answering', async () => {
    const { relay, sockets } = channel()
    const down: string[] = []
    relay.onDown = (reason) => down.push(reason)
    const connected = relay.connect()
    sockets[0].opened()
    sockets[0].close()
    await expect(connected).rejects.toThrow('did not let this device in')
    expect(down).toEqual(['the machine did not let this device in'])
  })

  test('dials again on the next request after the connection drops', async () => {
    const { relay, sockets } = channel()
    const connected = relay.connect()
    sockets[0].opened()
    sockets[0].receive(t.reply)
    await connected
    sockets[0].close()
    relay.send({ type: 'close', id: 9 })
    expect(sockets).toHaveLength(2)
  })
})
