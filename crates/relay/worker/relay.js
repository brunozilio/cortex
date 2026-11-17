const OPEN = 1
const DATA = 2
const CLOSE = 3
const HEADER = 5
const MOST_PAYLOAD = 65535
const MOST_PHONES = 8
const MACHINE_ID = /^[0-9a-f]{32}$/
const SOCKET_OPEN = 1
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
    return open(this.ctx.getWebSockets('machine'))[0]
  }

  phone(channel) {
    return open(this.ctx.getWebSockets(`phone:${channel}`))[0]
  }

  async fetch(request) {
    if (request.headers.get('upgrade') !== 'websocket') return failure(426, 'upgrade', 'This address takes a WebSocket.')
    const role = new URL(request.url).searchParams.get('role')
    const [client, server] = Object.values(new WebSocketPair())
    if (role === 'machine') {
      if (this.machine()) return failure(409, 'taken', 'Another machine is connected under that id.')
      this.ctx.acceptWebSocket(server, ['machine'])
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

  webSocketMessage(socket, message) {
    if (typeof message === 'string') return quietly(() => socket.close(1003, 'binary frames only'))
    if (this.ctx.getTags(socket).includes('machine')) return this.fromMachine(socket, message)
    if (message.byteLength > MOST_PAYLOAD) return quietly(() => socket.close(1009, 'too large'))
    const machine = this.machine()
    if (!machine) return quietly(() => socket.close(1011, 'the machine left'))
    machine.send(frame(DATA, socket.deserializeAttachment().channel, message))
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
        forwarded.search = `?role=${role}`
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
