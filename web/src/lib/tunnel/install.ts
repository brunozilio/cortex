import { generateKey, keyFrom, type StaticKey } from './noise'
import { machineId, pairedInitiator, pairingInitiator, parsePairing, type PairingLink } from './link'
import type { Answer, AnswerChannel, Ask } from './messages'
import { Tunnel } from './mux'
import { RelayChannel, type Greeting, type SocketLike } from './relay'
import { keepMachine, loadMachine, type PairedMachine } from './store'
import { utf8 } from './bytes'

export const MARKER = 'meta[name="remote-relay"]'

export type RemoteState =
  | { kind: 'connecting'; name?: string }
  | { kind: 'connected'; name: string }
  | { kind: 'unpaired' }
  | { kind: 'refused'; reason: string }
  | { kind: 'down'; reason: string }

export function onRelay(document: Pick<Document, 'querySelector'>): boolean {
  return document.querySelector(MARKER) !== null
}

export function deviceName(userAgent: string): string {
  const known: [RegExp, string][] = [
    [/iPhone/, 'iPhone'],
    [/iPad/, 'iPad'],
    [/Android.*Mobile/, 'Android phone'],
    [/Android/, 'Android tablet'],
    [/Macintosh/, 'Mac'],
    [/Windows/, 'Windows PC'],
    [/Linux/, 'Linux computer'],
  ]
  return known.find(([pattern]) => pattern.test(userAgent))?.[1] ?? 'Browser'
}

class Deferred implements AnswerChannel {
  onAnswer: (answer: Answer) => void = () => undefined
  onDown: (reason: string) => void = () => undefined
  private channel: AnswerChannel | null = null
  private failure: string | null = null
  private queued: Ask[] = []

  settle(channel: AnswerChannel | null, failure?: string) {
    if (channel) {
      channel.onAnswer = (answer) => this.onAnswer(answer)
      channel.onDown = (reason) => this.onDown(reason)
      this.channel = channel
      for (const ask of this.queued.splice(0)) channel.send(ask)
      return
    }
    this.failure = failure ?? 'not paired'
    if (this.queued.splice(0).length > 0) this.onDown(this.failure)
  }

  send(ask: Ask) {
    if (this.channel) return this.channel.send(ask)
    if (this.failure) return this.onDown(this.failure)
    this.queued.push(ask)
  }
}

function socketUrl(location: Location, machine: string): string {
  const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${scheme}//${location.host}/phone?machine=${machine}`
}

function greeter(device: StaticKey, machine: Uint8Array, pairing: PairingLink | null, name: string): () => Greeting {
  let first = pairing
  return () => {
    const link = first
    first = null
    if (link) return { initiator: pairingInitiator(device, link), payload: utf8(name), confirms: true }
    return { initiator: pairedInitiator(device, machine), payload: new Uint8Array(0) }
  }
}

async function prepare(show: (state: RemoteState) => void, open: (url: string) => SocketLike): Promise<RelayChannel | null> {
  const pairing = parsePairing(window.location.hash)
  const stored = await loadMachine().catch(() => undefined)
  if (!pairing && !stored) {
    show({ kind: 'unpaired' })
    return null
  }
  const name = stored && !pairing ? stored.name : deviceName(navigator.userAgent)
  const device = stored && !pairing ? keyFrom(stored.deviceSecret) : generateKey()
  const machine = pairing ? pairing.machine : (stored as PairedMachine).machine
  const id = machineId(machine)
  show({ kind: 'connecting', name })
  const channel = new RelayChannel({
    open,
    socketUrl: socketUrl(window.location, id),
    greet: greeter(device, machine, pairing, name),
    onUp: () => show({ kind: 'connected', name }),
  })
  try {
    await channel.connect()
  } catch (error) {
    show({ kind: 'refused', reason: error instanceof Error ? error.message : String(error) })
    return null
  }
  if (pairing) {
    await keepMachine({ machine, machineId: id, deviceSecret: device.secret, name, pairedAt: Date.now() }).catch(() => undefined)
    await navigator.storage?.persist?.().catch(() => false)
    history.replaceState(null, '', window.location.pathname + window.location.search)
  }
  const lost = channel.onDown
  channel.onDown = (reason) => {
    show({ kind: 'down', reason })
    lost(reason)
  }
  return channel
}

export function installTunnel(show: (state: RemoteState) => void): Tunnel {
  const deferred = new Deferred()
  const tunnel = new Tunnel(deferred)
  const Native = globalThis.WebSocket
  void prepare(show, (url) => new Native(url) as SocketLike).then(
    (channel) => deferred.settle(channel, 'this page is not paired with a machine'),
    (error: unknown) => deferred.settle(null, String(error)),
  )
  const scope = globalThis as unknown as Record<string, unknown>
  scope.fetch = (input: RequestInfo | URL, init?: RequestInit) => tunnel.fetch(input, init)
  scope.EventSource = function TunnelEvents(url: string | URL) {
    return tunnel.events(url)
  }
  scope.WebSocket = Object.assign(
    function TunnelWebSocket(url: string | URL) {
      return tunnel.socket(url)
    },
    { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 },
  )
  return tunnel
}
