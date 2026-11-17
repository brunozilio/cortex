import { x25519 } from '@noble/curves/ed25519'
import { chacha20poly1305 } from '@noble/ciphers/chacha'
import { sha256 } from '@noble/hashes/sha256'
import { hmac } from '@noble/hashes/hmac'
import { concat, random, utf8 } from './bytes'

export const IK = 'Noise_IK_25519_ChaChaPoly_SHA256'
export const IK_PSK2 = 'Noise_IKpsk2_25519_ChaChaPoly_SHA256'
const KEY = 32
const TAG = 16
const EMPTY = new Uint8Array(0)

export interface StaticKey {
  secret: Uint8Array
  public: Uint8Array
}

export function keyFrom(secret: Uint8Array): StaticKey {
  return { secret, public: x25519.getPublicKey(secret) }
}

export function generateKey(): StaticKey {
  return keyFrom(random(KEY))
}

function nonce(counter: number): Uint8Array {
  const bytes = new Uint8Array(12)
  new DataView(bytes.buffer).setBigUint64(4, BigInt(counter), true)
  return bytes
}

export class CipherState {
  private key: Uint8Array | null
  private counter = 0

  constructor(key: Uint8Array | null = null) {
    this.key = key
  }

  get keyed(): boolean {
    return this.key !== null
  }

  rekey(key: Uint8Array) {
    this.key = key
    this.counter = 0
  }

  encrypt(ad: Uint8Array, plain: Uint8Array): Uint8Array {
    if (!this.key) return plain
    return chacha20poly1305(this.key, nonce(this.counter++), ad).encrypt(plain)
  }

  decrypt(ad: Uint8Array, sealed: Uint8Array): Uint8Array {
    if (!this.key) return sealed
    const plain = chacha20poly1305(this.key, nonce(this.counter), ad).decrypt(sealed)
    this.counter++
    return plain
  }
}

function hkdf(chainingKey: Uint8Array, input: Uint8Array, outputs: 2 | 3): Uint8Array[] {
  const temp = hmac(sha256, chainingKey, input)
  const first = hmac(sha256, temp, Uint8Array.of(1))
  const second = hmac(sha256, temp, concat(first, Uint8Array.of(2)))
  if (outputs === 2) return [first, second]
  return [first, second, hmac(sha256, temp, concat(second, Uint8Array.of(3)))]
}

class SymmetricState {
  chainingKey: Uint8Array
  hash: Uint8Array
  cipher = new CipherState()

  constructor(protocol: string) {
    const name = utf8(protocol)
    this.hash = name.length <= KEY ? concat(name, new Uint8Array(KEY - name.length)) : sha256(name)
    this.chainingKey = this.hash
  }

  mixHash(data: Uint8Array) {
    this.hash = sha256(concat(this.hash, data))
  }

  mixKey(input: Uint8Array) {
    const [chainingKey, key] = hkdf(this.chainingKey, input, 2)
    this.chainingKey = chainingKey
    this.cipher.rekey(key)
  }

  mixKeyAndHash(input: Uint8Array) {
    const [chainingKey, hash, key] = hkdf(this.chainingKey, input, 3)
    this.chainingKey = chainingKey
    this.mixHash(hash)
    this.cipher.rekey(key)
  }

  encryptAndHash(plain: Uint8Array): Uint8Array {
    const sealed = this.cipher.encrypt(this.hash, plain)
    this.mixHash(sealed)
    return sealed
  }

  decryptAndHash(sealed: Uint8Array): Uint8Array {
    const plain = this.cipher.decrypt(this.hash, sealed)
    this.mixHash(sealed)
    return plain
  }

  split(): [CipherState, CipherState] {
    const [sending, receiving] = hkdf(this.chainingKey, EMPTY, 2)
    return [new CipherState(sending), new CipherState(receiving)]
  }
}

export class HandshakeError extends Error {}

export class Initiator {
  private readonly state: SymmetricState
  private readonly ephemeral: StaticKey
  private sent = false

  constructor(
    private readonly header: Uint8Array,
    private readonly device: StaticKey,
    private readonly machine: Uint8Array,
    private readonly psk: Uint8Array | null,
    ephemeral?: Uint8Array,
  ) {
    this.state = new SymmetricState(psk ? IK_PSK2 : IK)
    this.state.mixHash(header)
    this.state.mixHash(machine)
    this.ephemeral = ephemeral ? keyFrom(ephemeral) : generateKey()
  }

  hello(payload: Uint8Array): Uint8Array {
    if (this.sent) throw new HandshakeError('the first message was already written')
    this.sent = true
    const { state, ephemeral, device, machine } = this
    state.mixHash(ephemeral.public)
    if (this.psk) state.mixKey(ephemeral.public)
    state.mixKey(x25519.getSharedSecret(ephemeral.secret, machine))
    const sealedStatic = state.encryptAndHash(device.public)
    state.mixKey(x25519.getSharedSecret(device.secret, machine))
    const sealedPayload = state.encryptAndHash(payload)
    return concat(this.header, ephemeral.public, sealedStatic, sealedPayload)
  }

  finish(reply: Uint8Array): Transport {
    if (reply.length < KEY + TAG) throw new HandshakeError('the reply is too short')
    const { state, ephemeral, device } = this
    const remote = reply.subarray(0, KEY)
    state.mixHash(remote)
    if (this.psk) state.mixKey(remote)
    state.mixKey(x25519.getSharedSecret(ephemeral.secret, remote))
    state.mixKey(x25519.getSharedSecret(device.secret, remote))
    if (this.psk) state.mixKeyAndHash(this.psk)
    try {
      state.decryptAndHash(reply.subarray(KEY))
    } catch {
      throw new HandshakeError('the machine did not answer as the one paired with')
    }
    const [sending, receiving] = state.split()
    return new Transport(sending, receiving)
  }
}

export const MOST_MESSAGE_BYTES = 16 * 1024 * 1024
const PIECE_BYTES = 65_535 - TAG - 1
const LAST = 0
const MORE = 1

export class Transport {
  private partial: Uint8Array[] = []
  private partialBytes = 0

  constructor(
    private readonly sending: CipherState,
    private readonly receiving: CipherState,
  ) {}

  seal(message: Uint8Array): Uint8Array[] {
    if (message.length > MOST_MESSAGE_BYTES) throw new Error('a message is too large')
    const pieces: Uint8Array[] = []
    let at = 0
    do {
      const piece = message.subarray(at, at + PIECE_BYTES)
      at += piece.length
      const marker = at < message.length ? MORE : LAST
      pieces.push(this.sending.encrypt(EMPTY, concat(Uint8Array.of(marker), piece)))
    } while (at < message.length)
    return pieces
  }

  open(sealed: Uint8Array): Uint8Array | null {
    const plain = this.receiving.decrypt(EMPTY, sealed)
    if (plain.length === 0) throw new Error('a message opened without its marker')
    const marker = plain[0]
    this.partialBytes += plain.length - 1
    if (this.partialBytes > MOST_MESSAGE_BYTES) throw new Error('a message is too large')
    this.partial.push(plain.subarray(1))
    if (marker === MORE) return null
    if (marker !== LAST) throw new Error('a message opened with an unknown marker')
    const whole = concat(...this.partial)
    this.partial = []
    this.partialBytes = 0
    return whole
  }
}
