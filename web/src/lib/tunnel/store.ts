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
    const request = act(db.transaction(STORE, mode).objectStore(STORE))
    request.onsuccess = () => resolve(request.result as T)
    request.onerror = () => reject(request.error)
  }).finally(() => db.close())
}

export function loadMachine(): Promise<PairedMachine | undefined> {
  return within('readonly', (store) => store.get(CURRENT))
}

export function keepMachine(machine: PairedMachine): Promise<unknown> {
  return within('readwrite', (store) => store.put(machine, CURRENT))
}

export function forgetMachine(): Promise<unknown> {
  return within('readwrite', (store) => store.delete(CURRENT))
}
