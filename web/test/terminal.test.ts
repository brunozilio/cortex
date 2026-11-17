import { describe, expect, test } from 'bun:test'
import { resizeMessage, screenSize, terminalUrl } from '../src/lib/terminal'

describe('resizeMessage', () => {
  test('a size is sent as the JSON the console reads: whole columns and rows', () => {
    const message = resizeMessage({ cols: 100, rows: 30 })
    expect(message).toBe('{"cols":100,"rows":30}')
    expect(JSON.parse(message ?? '')).toEqual({ cols: 100, rows: 30 })
  })

  test('only the two numbers are sent, whatever else the size object carries', () => {
    const size = { cols: 80, rows: 24, pixelWidth: 640 } as { cols: number; rows: number }
    expect(resizeMessage(size)).toBe('{"cols":80,"rows":24}')
  })

  test('the largest size the console takes is sent, and one past it is not', () => {
    expect(resizeMessage({ cols: 65535, rows: 1 })).toBe('{"cols":65535,"rows":1}')
    expect(resizeMessage({ cols: 65536, rows: 1 })).toBeNull()
  })

  test('a size the console would refuse is not sent at all', () => {
    for (const size of [
      { cols: 0, rows: 24 },
      { cols: 80, rows: 0 },
      { cols: -1, rows: 24 },
      { cols: 80.5, rows: 24 },
      { cols: Number.NaN, rows: 24 },
      { cols: 80, rows: Number.POSITIVE_INFINITY },
    ]) {
      expect(resizeMessage(size)).toBeNull()
    }
  })
})

describe('terminalUrl', () => {
  test('the socket is opened on the page\'s own host, for that session, at that size', () => {
    const url = terminalUrl({ protocol: 'http:', host: '127.0.0.1:4000' }, 'abc-123', { cols: 120, rows: 40 })
    expect(url).toBe('ws://127.0.0.1:4000/api/terminal?session=abc-123&cols=120&rows=40')
  })

  test('a page served over TLS opens a secure socket, and the session is escaped', () => {
    const url = terminalUrl({ protocol: 'https:', host: 'localhost:4000' }, 'a b&c', { cols: 80, rows: 24 })
    expect(url).toBe('wss://localhost:4000/api/terminal?session=a+b%26c&cols=80&rows=24')
  })
})

describe('screenSize', () => {
  test('the size of the session\'s own screen is read from the console\'s text message', () => {
    expect(screenSize('{"cols":120,"rows":40}')).toEqual({ cols: 120, rows: 40 })
  })

  test('anything else is not a size', () => {
    for (const message of ['', 'hello', '[]', 'null', '{"cols":0,"rows":40}', '{"cols":"80","rows":24}', '{"cols":80}']) {
      expect(screenSize(message)).toBeNull()
    }
  })
})
