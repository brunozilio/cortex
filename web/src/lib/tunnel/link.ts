import { sha256 } from '@noble/hashes/sha256'
import { concat, fromBase64Url, toHex, utf8 } from './bytes'
import { Initiator, type StaticKey } from './noise'

const PREFIX = 'pair='
const PAIRED = 1
const PAIRING = 2

export interface PairingLink {
  machine: Uint8Array
  code: Uint8Array
}

export function parsePairing(fragment: string): PairingLink | null {
  const body = fragment.replace(/^#/, '')
  if (!body.startsWith(PREFIX)) return null
  const [machine, code, extra] = body.slice(PREFIX.length).split('.')
  if (!machine || !code || extra !== undefined) return null
  try {
    const link = { machine: fromBase64Url(machine), code: fromBase64Url(code) }
    return link.machine.length === 32 && link.code.length === 32 ? link : null
  } catch {
    return null
  }
}

export function pastedPairing(pasted: string): string | null {
  const text = pasted.trim()
  const at = text.indexOf('#')
  const fragment = at >= 0 ? text.slice(at) : `#${text}`
  return parsePairing(fragment) ? fragment : null
}

export function pairingId(code: Uint8Array): Uint8Array {
  return sha256(concat(utf8('pairing-id'), code)).subarray(0, 8)
}

export function machineId(machine: Uint8Array): string {
  return toHex(sha256(machine).subarray(0, 16))
}

export function pairingInitiator(device: StaticKey, link: PairingLink, ephemeral?: Uint8Array): Initiator {
  const header = concat(Uint8Array.of(PAIRING), pairingId(link.code))
  return new Initiator(header, device, link.machine, sha256(link.code), ephemeral)
}

export function pairedInitiator(device: StaticKey, machine: Uint8Array, ephemeral?: Uint8Array): Initiator {
  return new Initiator(Uint8Array.of(PAIRED), device, machine, null, ephemeral)
}
