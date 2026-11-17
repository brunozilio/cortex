import { describe, expect, test } from 'bun:test'
import { deviceName, onRelay } from '../src/lib/tunnel/install'

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
