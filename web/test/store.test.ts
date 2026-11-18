import { describe, expect, test } from 'bun:test'
import { keepMachine, loadMachine, read, written, type PairedMachine, type Shelf, type TextShelf } from '../src/lib/tunnel/store'

const paired: PairedMachine = {
  machine: Uint8Array.from({ length: 32 }, (_, at) => at),
  machineId: 'ab12',
  deviceSecret: Uint8Array.from({ length: 32 }, (_, at) => 255 - at),
  name: 'iPhone',
  pairedAt: 1_700_000_000_000,
}

function memory(): TextShelf & { kept: Map<string, string> } {
  const kept = new Map<string, string>()
  return { kept, getItem: (key) => kept.get(key) ?? null, setItem: (key, value) => void kept.set(key, value) }
}

function shelf(failures: number, held?: PairedMachine): Shelf & { reads: number; writes: PairedMachine[] } {
  const made = {
    reads: 0,
    writes: [] as PairedMachine[],
    read: async () => {
      made.reads += 1
      if (made.reads <= failures) throw new Error('Connection to Indexed Database server lost')
      return held
    },
    write: async (machine: PairedMachine) => void made.writes.push(machine),
  }
  return made
}

describe('the kept pairing', () => {
  test('it reads back what was written, bytes included', () => {
    expect(read(written(paired))).toEqual(paired)
    expect(read('{"machine":1}')).toBeUndefined()
    expect(read('not json')).toBeUndefined()
    expect(read(null)).toBeUndefined()
  })

  test('a database that fails for a moment is asked again before the page says it is not paired', async () => {
    const flaky = shelf(2, paired)
    expect(await loadMachine(flaky, memory(), 0)).toEqual(paired)
    expect(flaky.reads).toBe(3)
  })

  test('when the database lost it or keeps failing, the copy in local storage pairs the page and is put back', async () => {
    const mirror = memory()
    await keepMachine(paired, shelf(0), mirror)
    const lost = shelf(0)
    expect(await loadMachine(lost, mirror, 0)).toEqual(paired)
    expect(lost.writes).toEqual([paired])
    expect(await loadMachine(shelf(5), mirror, 0)).toEqual(paired)
  })

  test('keeping succeeds when either place took it, and fails only when neither did', async () => {
    const broken: Shelf = { read: async () => undefined, write: async () => Promise.reject(new Error('quota')) }
    const mirror = memory()
    await keepMachine(paired, broken, mirror)
    expect(mirror.kept.size).toBe(1)
    await expect(keepMachine(paired, broken, null)).rejects.toThrow('quota')
  })
})
