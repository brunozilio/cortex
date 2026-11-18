import { fromHex, toHex } from './bytes'

const DATABASE = 'remote-control'
const STORE = 'machines'
const CURRENT = 'current'

export interface PairedMachine {
  machine: Uint8Array
  machineId: string
  deviceSecret: Uint8Array
  name: string
  pairedAt: number
}

function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const opening = indexedDB.open(DATABASE, 1)
    opening.onupgradeneeded = () => opening.result.createObjectStore(STORE)
    opening.onsuccess = () => resolve(opening.result)
    opening.onerror = () => reject(opening.error)
  })
}

async function within<T>(mode: IDBTransactionMode, act: (store: IDBObjectStore) => IDBRequest): Promise<T> {
  const db = await database()
  return new Promise<T>((resolve, reject) => {
    const transaction = db.transaction(STORE, mode)
    const request = act(transaction.objectStore(STORE))
    transaction.oncomplete = () => resolve(request.result as T)
    transaction.onerror = () => reject(transaction.error ?? request.error)
    transaction.onabort = () => reject(transaction.error ?? request.error)
  }).finally(() => db.close())
}

export interface Shelf {
  read(): Promise<PairedMachine | undefined>
  write(machine: PairedMachine): Promise<unknown>
}

export interface TextShelf {
  getItem(key: string): string | null
  setItem(key: string, value: string): void
}

const MIRROR = 'remote-control.current'
const TRIES = 3

export function written(machine: PairedMachine): string {
  return JSON.stringify({ ...machine, machine: toHex(machine.machine), deviceSecret: toHex(machine.deviceSecret) })
}

export function read(text: string | null): PairedMachine | undefined {
  if (!text) return undefined
  try {
    const kept = JSON.parse(text) as Record<string, unknown>
    const { machine, machineId, deviceSecret, name, pairedAt } = kept
    if (typeof machine !== 'string' || typeof deviceSecret !== 'string' || typeof machineId !== 'string') return undefined
    if (typeof name !== 'string' || typeof pairedAt !== 'number') return undefined
    return { machine: fromHex(machine), machineId, deviceSecret: fromHex(deviceSecret), name, pairedAt }
  } catch {
    return undefined
  }
}

export const indexedShelf: Shelf = {
  read: async () => (typeof indexedDB === 'undefined' ? undefined : within('readonly', (store) => store.get(CURRENT))),
  write: (machine) => within('readwrite', (store) => store.put(machine, CURRENT)),
}

function localShelf(): TextShelf | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    return null
  }
}

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms))

export async function loadMachine(shelf: Shelf = indexedShelf, mirror: TextShelf | null = localShelf(), wait = 150): Promise<PairedMachine | undefined> {
  for (let attempt = 1; attempt <= TRIES; attempt += 1) {
    try {
      const found = await shelf.read()
      if (found) return found
      break
    } catch {
      if (attempt < TRIES) await pause(wait * attempt)
    }
  }
  let mirrored: PairedMachine | undefined
  try {
    mirrored = read(mirror?.getItem(MIRROR) ?? null)
  } catch {
    return undefined
  }
  if (mirrored) await shelf.write(mirrored).catch(() => undefined)
  return mirrored
}

export async function keepMachine(machine: PairedMachine, shelf: Shelf = indexedShelf, mirror: TextShelf | null = localShelf()): Promise<void> {
  let mirrored = false
  try {
    mirror?.setItem(MIRROR, written(machine))
    mirrored = mirror !== null
  } catch {
    mirrored = false
  }
  try {
    await shelf.write(machine)
  } catch (error) {
    if (!mirrored) throw error
  }
}
