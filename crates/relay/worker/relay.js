const OPEN = 1
const DATA = 2
const CLOSE = 3
const HEADER = 5
const MOST_PAYLOAD = 65535
const MOST_PHONES = 8
const MACHINE_ID = /^[0-9a-f]{32}$/
const SOCKET_OPEN = 1
const MOST_ASKING = 4
const PROOF_WAIT_MS = 10_000
const UNPROVEN_CLOSE = 4003
const TAKEN_CLOSE = 4009
const LABEL = new TextEncoder().encode('relay-machine-proof')
const TYPES = {
  html: 'text/html; charset=utf-8',
  js: 'text/javascript; charset=utf-8',
  css: 'text/css; charset=utf-8',
  json: 'application/json',
  png: 'image/png',
  svg: 'image/svg+xml',
  ico: 'image/x-icon',
  woff2: 'font/woff2',
  webmanifest: 'application/manifest+json',
  md: 'text/markdown; charset=utf-8',
  txt: 'text/plain; charset=utf-8',
}

function failure(status, code, message) {
  return new Response(JSON.stringify({ error: { code, message } }), { status, headers: { 'content-type': 'application/json' } })
}

function frame(kind, channel, payload) {
  const body = payload ? new Uint8Array(payload) : new Uint8Array(0)
  const out = new Uint8Array(HEADER + body.length)
  out[0] = kind
  new DataView(out.buffer).setUint32(1, channel)
  out.set(body, HEADER)
  return out
}

function hex(bytes) {
  return [...bytes].map((byte) => byte.toString(16).padStart(2, '0')).join('')
}

function unhex(text) {
  if (typeof text !== 'string' || text.length % 2 !== 0 || !/^[0-9a-f]*$/.test(text)) return null
  return Uint8Array.from(text.match(/../g) ?? [], (pair) => parseInt(pair, 16))
}

function joined(...parts) {
  const out = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let at = 0
  for (const part of parts) {
    out.set(part, at)
    at += part.length
  }
  return out
}

async function sha256(bytes) {
  return new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))
}

async function challenge() {
  const pair = await crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits'])
  const ephemeral = new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey))
  const secret = new Uint8Array(await crypto.subtle.exportKey('pkcs8', pair.privateKey))
  const nonce = crypto.getRandomValues(new Uint8Array(32))
  return { text: `challenge ${hex(ephemeral)} ${hex(nonce)}`, secret: hex(secret), nonce: hex(nonce) }
}

export async function proves(asked, id, answer) {
  const words = typeof answer === 'string' ? answer.split(' ') : []
  if (words.length !== 3 || words[0] !== 'proof') return false
  const machine = unhex(words[1])
  const given = unhex(words[2])
  if (!machine || machine.length !== 32 || !given || given.length !== 32) return false
  if (hex((await sha256(machine)).subarray(0, 16)) !== id) return false
  try {
    const secret = await crypto.subtle.importKey('pkcs8', unhex(asked.secret), { name: 'X25519' }, false, ['deriveBits'])
    const theirs = await crypto.subtle.importKey('raw', machine, { name: 'X25519' }, false, [])
    const shared = new Uint8Array(await crypto.subtle.deriveBits({ name: 'X25519', public: theirs }, secret, 256))
    const wanted = await sha256(joined(LABEL, shared, unhex(asked.nonce), new TextEncoder().encode(id)))
    return wanted.reduce((differ, byte, at) => differ | (byte ^ given[at]), 0) === 0
  } catch {
    return false
  }
}

function open(sockets) {
  return sockets.filter((socket) => socket.readyState === SOCKET_OPEN)
}

function quietly(act) {
  try {
    act()
  } catch {
    return undefined
  }
}

export class Relay {
  constructor(ctx) {
    this.ctx = ctx
    ctx.setWebSocketAutoResponse(new WebSocketRequestResponsePair('ping', 'pong'))
  }

  machine() {
    return open(this.ctx.getWebSockets('machine')).find((socket) => socket.deserializeAttachment()?.proven)
  }

  asking() {
    return open(this.ctx.getWebSockets('machine')).filter((socket) => !socket.deserializeAttachment()?.proven)
  }

  phone(channel) {
    return open(this.ctx.getWebSockets(`phone:${channel}`))[0]
  }

  async fetch(request) {
    if (request.headers.get('upgrade') !== 'websocket') return failure(426, 'upgrade', 'This address takes a WebSocket.')
    const asked = new URL(request.url).searchParams
    const role = asked.get('role')
    const [client, server] = Object.values(new WebSocketPair())
    if (role === 'machine') {
      if (this.machine()) return failure(409, 'taken', 'Another machine is connected under that id.')
      if (this.asking().length >= MOST_ASKING) return failure(429, 'full', 'Too many machines are proving this id.')
      const id = asked.get('id') ?? ''
      const sent = await challenge()
      this.ctx.acceptWebSocket(server, ['machine'])
      server.serializeAttachment({ proven: false, id, secret: sent.secret, nonce: sent.nonce, until: Date.now() + PROOF_WAIT_MS })
      server.send(sent.text)
      await this.ctx.storage?.setAlarm?.(Date.now() + PROOF_WAIT_MS)
      return new Response(null, { status: 101, webSocket: client })
    }
    const machine = this.machine()
    if (!machine) return failure(404, 'absent', 'That machine is not connected to the relay.')
    if (open(this.ctx.getWebSockets('phone')).length >= MOST_PHONES) return failure(429, 'full', 'Too many phones on this machine.')
    let channel = 0
    while (channel === 0 || this.phone(channel)) channel = crypto.getRandomValues(new Uint32Array(1))[0]
    this.ctx.acceptWebSocket(server, ['phone', `phone:${channel}`])
    server.serializeAttachment({ channel })
    machine.send(frame(OPEN, channel))
    return new Response(null, { status: 101, webSocket: client })
  }

  async webSocketMessage(socket, message) {
    const sentByMachine = this.ctx.getTags(socket).includes('machine')
    const state = sentByMachine ? socket.deserializeAttachment() : null
    if (sentByMachine && !state?.proven) return this.proving(socket, state, message)
    if (typeof message === 'string') return quietly(() => socket.close(1003, 'binary frames only'))
    if (sentByMachine) return this.fromMachine(socket, message)
    if (message.byteLength > MOST_PAYLOAD) return quietly(() => socket.close(1009, 'too large'))
    const machine = this.machine()
    if (!machine) return quietly(() => socket.close(1011, 'the machine left'))
    machine.send(frame(DATA, socket.deserializeAttachment().channel, message))
  }

  async proving(socket, state, message) {
    if (!state || !(await proves(state, state.id, message))) return quietly(() => socket.close(UNPROVEN_CLOSE, 'the machine did not prove its key'))
    if (this.machine()) return quietly(() => socket.close(TAKEN_CLOSE, 'taken'))
    socket.serializeAttachment({ proven: true })
    socket.send('proven')
  }

  alarm() {
    const now = Date.now()
    for (const socket of this.asking()) {
      if ((socket.deserializeAttachment()?.until ?? 0) <= now) quietly(() => socket.close(UNPROVEN_CLOSE, 'the machine did not prove its key'))
    }
  }

  fromMachine(socket, message) {
    const bytes = new Uint8Array(message)
    if (bytes.length < HEADER || bytes.length > HEADER + MOST_PAYLOAD) return quietly(() => socket.close(1002, 'not a frame'))
    const channel = new DataView(message).getUint32(1)
    const phone = this.phone(channel)
    if (bytes[0] === DATA) return phone && quietly(() => phone.send(bytes.subarray(HEADER)))
    if (bytes[0] === CLOSE) return phone && quietly(() => phone.close(1000, 'closed by the machine'))
    quietly(() => socket.close(1002, 'not a frame'))
  }

  webSocketClose(socket, code) {
    this.left(socket)
    quietly(() => socket.close(code === 1005 ? 1000 : code))
  }

  webSocketError(socket) {
    this.left(socket)
  }

  left(socket) {
    if (this.ctx.getTags(socket).includes('machine')) {
      if (!socket.deserializeAttachment()?.proven) return
      for (const phone of open(this.ctx.getWebSockets('phone'))) quietly(() => phone.close(1001, 'the machine left'))
      return
    }
    const machine = this.machine()
    if (machine) quietly(() => machine.send(frame(CLOSE, socket.deserializeAttachment().channel)))
  }
}

function pageFile(page, headers, pathname) {
  const wanted = pathname.replace(/^\/+/, '')
  const path = wanted in page ? wanted : 'index.html'
  const body = page[path]
  if (!body) return new Response('The page was not built into this relay.', { status: 503 })
  const extension = path.split('.').pop()
  const fresh = !pathname.startsWith('/assets/')
  return new Response(body, {
    headers: {
      ...headers,
      'content-type': TYPES[extension] ?? 'application/octet-stream',
      'cache-control': fresh ? 'no-cache' : 'public, max-age=31536000, immutable',
    },
  })
}

export function makeWorker({ page, headers, manifest }) {
  return {
    async fetch(request, env) {
      const url = new URL(request.url)
      if (url.pathname === '/machine' || url.pathname === '/phone') {
        const role = url.pathname.slice(1)
        const id = url.searchParams.get(role === 'machine' ? 'id' : 'machine') ?? ''
        if (!MACHINE_ID.test(id)) return failure(400, 'validation', 'A machine id is 32 lowercase hex digits.')
        const forwarded = new URL(request.url)
        forwarded.search = `?role=${role}&id=${id}`
        return env.RELAY.get(env.RELAY.idFromName(id)).fetch(new Request(forwarded, request))
      }
      if (url.pathname === '/relay.json') return Response.json({ relay: true })
      if (url.pathname === '/manifest.webmanifest') {
        return new Response(manifest, { headers: { 'content-type': 'application/manifest+json', 'cache-control': 'no-cache' } })
      }
      if (url.pathname.startsWith('/api/')) return failure(404, 'relay', 'The relay has no API; it forwards to your machine.')
      return pageFile(page, headers, url.pathname)
    },
  }
}
