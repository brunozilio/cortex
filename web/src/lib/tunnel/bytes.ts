export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0))
  let at = 0
  for (const part of parts) {
    out.set(part, at)
    at += part.length
  }
  return out
}

export function utf8(text: string): Uint8Array {
  return new TextEncoder().encode(text)
}

export function fromHex(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2)
  for (let at = 0; at < out.length; at++) out[at] = parseInt(hex.slice(at * 2, at * 2 + 2), 16)
  return out
}

export function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}

export function fromBase64Url(text: string): Uint8Array {
  const standard = text.replace(/-/g, '+').replace(/_/g, '/')
  return fromBase64(standard + '='.repeat((4 - (standard.length % 4)) % 4))
}

export function fromBase64(text: string): Uint8Array {
  const binary = atob(text)
  return Uint8Array.from(binary, (letter) => letter.charCodeAt(0))
}

export function toBase64(bytes: Uint8Array): string {
  let binary = ''
  for (let at = 0; at < bytes.length; at += 0x8000) binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000))
  return btoa(binary)
}

export function random(length: number): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(length))
}
