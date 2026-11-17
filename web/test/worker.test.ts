import { afterAll, beforeAll, describe, expect, test } from 'bun:test'
import transcript from './fixtures/noise.json'
import { fromHex, toHex } from '../src/lib/tunnel/bytes'
import { Relay, makeWorker, proves } from '../../crates/relay/worker/relay.js'

class FakeSocket {
  readyState = 1
  sent: (Uint8Array | string)[] = []
  closed: number | null = null
  private attachment: unknown = null
  send(data: Uint8Array | string) {
    this.sent.push(data)
  }
  close(code = 1000) {
    this.readyState = 3
    this.closed = code
  }
  serializeAttachment(value: unknown) {
    this.attachment = value
  }
  deserializeAttachment() {
    return this.attachment
  }
}

class FakeResponse {
  constructor(
    readonly body: unknown,
    readonly init: { status?: number; headers?: Record<string, string>; webSocket?: FakeSocket } = {},
  ) {}
  get status() {
    return this.init.status ?? 200
  }
  static json(value: unknown) {
    return new FakeResponse(JSON.stringify(value), { headers: { 'content-type': 'application/json' } })
  }
}

class FakeContext {
  accepted: { socket: FakeSocket; tags: string[] }[] = []
  setWebSocketAutoResponse() {}
  acceptWebSocket(socket: FakeSocket, tags: string[]) {
    this.accepted.push({ socket, tags })
  }
  getWebSockets(tag?: string) {
    return this.accepted.filter((entry) => !tag || entry.tags.includes(tag)).map((entry) => entry.socket)
  }
  getTags(socket: FakeSocket) {
    return this.accepted.find((entry) => entry.socket === socket)?.tags ?? []
  }
}

const saved = { Response: globalThis.Response }

afterAll(() => {
  globalThis.Response = saved.Response
})

beforeAll(() => {
  const scope = globalThis as Record<string, unknown>
  scope.Response = FakeResponse
  scope.WebSocketPair = function () {
    return { 0: new FakeSocket(), 1: new FakeSocket() }
  }
  scope.WebSocketRequestResponsePair = function () {}
})

const upgrade = (role: string, id = '') => new Request(`https://relay.example/x?role=${role}&id=${id}`, { headers: { upgrade: 'websocket' } })

const LABEL = new TextEncoder().encode('relay-machine-proof')
const PKCS8_X25519 = '302e020100300506032b656e04220420'

async function machineKey() {
  const pair = (await crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits'])) as CryptoKeyPair
  const ours = new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey))
  const id = toHex(new Uint8Array(await crypto.subtle.digest('SHA-256', ours)).subarray(0, 16))
  const answer = async (challenge: string) => {
    const [, ephemeral, nonce] = challenge.split(' ')
    const theirs = await crypto.subtle.importKey('raw', fromHex(ephemeral), { name: 'X25519' }, false, [])
    const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'X25519', public: theirs }, pair.privateKey, 256))
    const parts = [LABEL, shared, fromHex(nonce), new TextEncoder().encode(id)]
    const all = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
    parts.reduce((at, part) => (all.set(part, at), at + part.length), 0)
    return `proof ${toHex(ours)} ${toHex(new Uint8Array(await crypto.subtle.digest('SHA-256', all)))}`
  }
  return { id, answer }
}

async function proven(relay: Relay, ctx: FakeContext, key: Awaited<ReturnType<typeof machineKey>>) {
  const answer = (await relay.fetch(upgrade('machine', key.id))) as unknown as FakeResponse
  const machine = ctx.getWebSockets('machine').at(-1) as FakeSocket
  await relay.webSocketMessage(machine, await key.answer(machine.sent[0] as string))
  expect(machine.sent[1]).toBe('proven')
  machine.sent.length = 0
  return { machine, answer }
}

function frame(kind: number, channel: number, payload: number[] = []) {
  const out = new Uint8Array(5 + payload.length)
  out[0] = kind
  new DataView(out.buffer).setUint32(1, channel)
  out.set(payload, 5)
  return out
}

function decode(bytes: Uint8Array) {
  return { kind: bytes[0], channel: new DataView(bytes.buffer, bytes.byteOffset).getUint32(1), payload: [...bytes.subarray(5)] }
}

async function room() {
  const ctx = new FakeContext()
  const relay = new Relay(ctx)
  const { machine, answer: machineAnswer } = await proven(relay, ctx, await machineKey())
  return { ctx, relay, machine, machineAnswer }
}

describe('the relay Worker pairs a phone with its machine as the Rust relay does', () => {
  test('one machine per id; a phone needs its machine', async () => {
    const ctx = new FakeContext()
    const relay = new Relay(ctx)
    const key = await machineKey()
    expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(404)
    expect((await proven(relay, ctx, key)).answer.status).toBe(101)
    expect(((await relay.fetch(upgrade('machine', key.id))) as unknown as FakeResponse).status).toBe(409)
  })

  test('a machine that cannot prove the key behind its id never takes its place', async () => {
    const ctx = new FakeContext()
    const relay = new Relay(ctx)
    const owner = await machineKey()
    const impostor = await machineKey()
    await relay.fetch(upgrade('machine', owner.id))
    const asking = ctx.getWebSockets('machine')[0]
    expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(404)
    await relay.webSocketMessage(asking, await impostor.answer(asking.sent[0] as string))
    expect(asking.closed).toBe(4003)
    await relay.fetch(upgrade('machine', owner.id))
    const binary = ctx.getWebSockets('machine')[1]
    await relay.webSocketMessage(binary, frame(2, 1).buffer)
    expect(binary.closed).toBe(4003)
    await proven(relay, ctx, owner)
    expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(101)
  })

  test('the Worker accepts exactly the proof the Rust machine writes', async () => {
    const proof = transcript.proof
    const asked = { secret: PKCS8_X25519 + proof.ephemeral_private, nonce: proof.nonce }
    expect(await proves(asked, proof.machine_id, proof.proof)).toBe(true)
    expect(await proves(asked, proof.machine_id.replace(/^./, '0'), proof.proof)).toBe(false)
    expect(await proves({ ...asked, nonce: '00'.repeat(32) }, proof.machine_id, proof.proof)).toBe(false)
  })

  test('a phone gets a channel; bytes flow both ways in frames; closes reach the other side', async () => {
    const { ctx, relay, machine } = await room()
    expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(101)
    const phone = ctx.getWebSockets('phone')[0]
    const opened = decode(machine.sent[0] as Uint8Array)
    expect(opened.kind).toBe(1)
    relay.webSocketMessage(phone, Uint8Array.of(7, 8).buffer)
    expect(decode(machine.sent[1] as Uint8Array)).toEqual({ kind: 2, channel: opened.channel, payload: [7, 8] })
    relay.webSocketMessage(machine, frame(2, opened.channel, [9]).buffer)
    expect([...(phone.sent[0] as Uint8Array)]).toEqual([9])
    relay.webSocketMessage(machine, frame(3, opened.channel).buffer)
    expect(phone.closed).toBe(1000)
    const { ctx: other, relay: second, machine: owner } = await room()
    await second.fetch(upgrade('phone'))
    const leaving = other.getWebSockets('phone')[0]
    const channel = decode(owner.sent[0] as Uint8Array).channel
    leaving.readyState = 3
    second.webSocketClose(leaving, 1000)
    expect(decode(owner.sent.at(-1) as Uint8Array)).toEqual({ kind: 3, channel, payload: [] })
  })

  test('phones are closed when the machine leaves, and text or oversized frames close the sender', async () => {
    const { ctx, relay, machine } = await room()
    await relay.fetch(upgrade('phone'))
    const phone = ctx.getWebSockets('phone')[0]
    relay.webSocketMessage(phone, new Uint8Array(65536).buffer)
    expect(phone.closed).toBe(1009)
    await relay.fetch(upgrade('phone'))
    const next = ctx.getWebSockets('phone').find((socket) => socket.readyState === 1) as FakeSocket
    machine.readyState = 3
    relay.webSocketClose(machine, 1000)
    expect(next.closed).toBe(1001)
    const { relay: fresh, machine: talker } = await room()
    fresh.webSocketMessage(talker, 'hello')
    expect(talker.closed).toBe(1003)
  })

  test('no more than eight phones per machine', async () => {
    const { relay } = await room()
    for (let at = 0; at < 8; at++) expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(101)
    expect(((await relay.fetch(upgrade('phone'))) as unknown as FakeResponse).status).toBe(429)
  })
})

describe('the Worker routes sockets to the machine\'s object and serves the page', () => {
  const page = { 'index.html': new TextEncoder().encode('<html>'), 'assets/a.js': new TextEncoder().encode('a()') }
  const worker = makeWorker({ page, headers: { 'content-security-policy': "default-src 'self'" }, manifest: '{"name":"App"}' })
  const id = '0123456789abcdef0123456789abcdef'
  const routed: string[] = []
  const env = {
    RELAY: {
      idFromName: (name: string) => name,
      get: (name: string) => ({
        fetch: (request: Request) => {
          routed.push(`${name} ${new URL(request.url).search}`)
          return new FakeResponse(null, { status: 101 })
        },
      }),
    },
  }

  test('a socket reaches the object named after the machine, with its role fixed by the path', async () => {
    await worker.fetch(new Request(`https://r.example/phone?machine=${id}&role=machine`), env)
    await worker.fetch(new Request(`https://r.example/machine?id=${id}`), env)
    expect(routed).toEqual([`${id} ?role=phone&id=${id}`, `${id} ?role=machine&id=${id}`])
    const bad = (await worker.fetch(new Request('https://r.example/phone?machine=nope'), env)) as unknown as FakeResponse
    expect(bad.status).toBe(400)
  })

  test('the page, its manifest and its headers; no API', async () => {
    const index = (await worker.fetch(new Request('https://r.example/sessions/x'), env)) as unknown as FakeResponse
    expect(index.init.headers?.['content-type']).toContain('text/html')
    expect(index.init.headers?.['cache-control']).toBe('no-cache')
    expect(index.init.headers?.['content-security-policy']).toBe("default-src 'self'")
    const asset = (await worker.fetch(new Request('https://r.example/assets/a.js'), env)) as unknown as FakeResponse
    expect(asset.init.headers?.['cache-control']).toContain('immutable')
    const manifest = (await worker.fetch(new Request('https://r.example/manifest.webmanifest'), env)) as unknown as FakeResponse
    expect(manifest.body).toBe('{"name":"App"}')
    const api = (await worker.fetch(new Request('https://r.example/api/report'), env)) as unknown as FakeResponse
    expect(api.status).toBe(404)
  })
})
