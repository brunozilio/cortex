import { describe, expect, test } from 'bun:test'
import { deviceName, installTunnel, onRelay } from '../src/lib/tunnel/install'
import { toHex } from '../src/lib/tunnel/bytes'
import { generateKey } from '../src/lib/tunnel/noise'

describe('the page knows where it runs and who it is', () => {
  test('only a page the relay marked talks through the tunnel', () => {
    expect(onRelay({ querySelector: (selector: string) => (selector === 'meta[name="remote-relay"]' ? ({} as Element) : null) })).toBe(true)
    expect(onRelay({ querySelector: () => null })).toBe(false)
  })

  test('a device names itself after what it is, never after its owner', () => {
    expect(deviceName('Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X)')).toBe('iPhone')
    expect(deviceName('Mozilla/5.0 (Linux; Android 15; Pixel 9) Mobile Safari')).toBe('Android phone')
    expect(deviceName('Mozilla/5.0 (Linux; Android 15; Tab)')).toBe('Android tablet')
    expect(deviceName('Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0)')).toBe('Mac')
    expect(deviceName('curl/8')).toBe('Browser')
  })
})

describe('installing the tunnel', () => {
  test('the relay connection uses the browser\'s own WebSocket, not the tunnel that replaces it', async () => {
    const scope = globalThis as Record<string, unknown>
    const saved = { WebSocket: scope.WebSocket, fetch: scope.fetch, EventSource: scope.EventSource, window: scope.window }
    const opened: string[] = []
    class Native {
      binaryType = 'blob'
      readyState = 0
      onopen = null
      onmessage = null
      onclose = null
      onerror = null
      constructor(url: string) {
        opened.push(url)
      }
      send() {}
      close() {}
    }
    const machine = generateKey().public
    const base64url = (bytes: Uint8Array) => Buffer.from(bytes).toString('base64url')
    scope.WebSocket = Native
    scope.window = { location: { hash: `#pair=${base64url(machine)}.${base64url(new Uint8Array(32).fill(1))}`, host: 'relay.example', protocol: 'https:', pathname: '/', search: '' } }
    try {
      installTunnel(() => undefined)
      expect(scope.WebSocket).not.toBe(Native)
      for (let tick = 0; tick < 20 && opened.length === 0; tick++) await new Promise((wake) => setTimeout(wake, 5))
      expect(opened).toHaveLength(1)
      expect(opened[0]).toStartWith('wss://relay.example/phone?machine=')
      expect(toHex(machine).length).toBe(64)
    } finally {
      Object.assign(scope, saved)
    }
  })
})
