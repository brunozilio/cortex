import { describe, expect, test } from 'bun:test'
import transcript from './fixtures/noise.json'
import { fromHex, toHex, utf8 } from '../src/lib/tunnel/bytes'
import { keyFrom, type Transport } from '../src/lib/tunnel/noise'
import { machineId, pairedInitiator, pairingInitiator, parsePairing, pastedPairing } from '../src/lib/tunnel/link'

type Exchange = { plain: string; sealed: string[] }

function opened(transport: Transport, exchange: Exchange): string | null {
  let whole: Uint8Array | null = null
  for (const piece of exchange.sealed) whole = transport.open(fromHex(piece))
  return whole ? toHex(whole) : null
}

describe('the page speaks the same Noise as the machine', () => {
  test('pairing: the first message, the reply and both directions match the Rust side byte for byte', () => {
    const t = transcript.pairing
    const link = parsePairing(t.fragment)
    expect(link).not.toBeNull()
    expect(toHex(link!.machine)).toBe(t.machine_public)
    const device = keyFrom(fromHex(t.device_private))
    expect(toHex(device.public)).toBe(t.device_public)
    const initiator = pairingInitiator(device, link!, fromHex(t.ephemeral_private))
    expect(toHex(initiator.hello(utf8(t.name)))).toBe(t.hello)
    const transport = initiator.finish(fromHex(t.reply))
    const ask = t.transport.phone_to_machine
    expect(transport.seal(fromHex(ask.plain)).map(toHex)).toEqual(ask.sealed)
    expect(opened(transport, t.transport.machine_to_phone)).toBe(t.transport.machine_to_phone.plain)
  })

  test('paired: a known phone reconnects with IK and reads a message split in pieces', () => {
    const t = transcript.paired
    expect(machineId(fromHex(t.machine_public))).toBe(t.machine_id)
    const initiator = pairedInitiator(keyFrom(fromHex(t.device_private)), fromHex(t.machine_public), fromHex(t.ephemeral_private))
    expect(toHex(initiator.hello(new Uint8Array(0)))).toBe(t.hello)
    const transport = initiator.finish(fromHex(t.reply))
    expect(transport.seal(fromHex(t.transport.phone_to_machine.plain)).map(toHex)).toEqual(t.transport.phone_to_machine.sealed)
    expect(opened(transport, t.transport.machine_to_phone)).toBe(t.transport.machine_to_phone.plain)
    const large = t.transport.machine_to_phone_large
    expect(large.sealed.length).toBe(2)
    expect(opened(transport, large)).toBe(large.plain)
  })

  test('a reply from another machine does not finish the handshake', () => {
    const t = transcript.paired
    const initiator = pairedInitiator(keyFrom(fromHex(t.device_private)), fromHex(t.machine_public))
    initiator.hello(new Uint8Array(0))
    expect(() => initiator.finish(fromHex(t.reply))).toThrow()
  })

  test('a piece changed on the way does not open', () => {
    const t = transcript.paired
    const initiator = pairedInitiator(keyFrom(fromHex(t.device_private)), fromHex(t.machine_public), fromHex(t.ephemeral_private))
    initiator.hello(new Uint8Array(0))
    const transport = initiator.finish(fromHex(t.reply))
    const piece = fromHex(t.transport.machine_to_phone.sealed[0])
    piece[3] ^= 1
    expect(() => transport.open(piece)).toThrow()
  })

  test('a link that is not a pairing link is refused', () => {
    expect(parsePairing('#other=1')).toBeNull()
    expect(parsePairing('#pair=AAAA.AAAA')).toBeNull()
    expect(parsePairing('#pair=abc')).toBeNull()
  })

  test('a pasted pairing link, whole or only its fragment, gives the fragment to pair with', () => {
    const fragment = `#${transcript.pairing.fragment.replace(/^#/, "")}`
    expect(pastedPairing(`  https://relay.example/${fragment}\n`)).toBe(fragment)
    expect(pastedPairing(fragment.slice(1))).toBe(fragment)
    expect(pastedPairing('https://relay.example/')).toBeNull()
    expect(pastedPairing('hello')).toBeNull()
  })
})
