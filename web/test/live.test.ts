import { describe, expect, test } from 'bun:test'
import { followSelection, mergeSession, refetch } from '../src/lib/live'
import { INSIGHTS, answers, report, run, session, span } from './make'

describe('mergeSession', () => {
  test('an update replaces its session in place and leaves the others and the totals alone', () => {
    const before = report([session('a', [run('a', 0)]), session('b', [run('b', 0)])])
    const update = session('b', [run('b', 0), run('b', 1)], INSIGHTS)

    const after = mergeSession(before, update)

    expect(after.sessions.map((entry) => [entry.id, entry.runs.length])).toEqual([['a', 1], ['b', 2]])
    expect(after.sessions[1]).toBe(update)
    expect(after.sessions[0]).toBe(before.sessions[0])
    expect([after.current, after.insights, after.generated_at_ms]).toEqual([before.current, before.insights, before.generated_at_ms])
    expect(before.sessions[1].runs.length).toBe(1)
  })

  test('a session the report did not know comes first', () => {
    const before = report([session('a', [run('a', 0)])])
    const after = mergeSession(before, session('new', [run('new', 0)]))
    expect(after.sessions.map((entry) => entry.id)).toEqual(['new', 'a'])
  })
})

describe('followSelection', () => {
  const two = [span({ id: 1, kind: 'run' }), span({ id: 2 })]

  test('watching the newest run of a session moves on to the run that follows it', () => {
    const before = report([session('a', [run('a', 0, { spans: two })])])
    const update = session('a', [run('a', 0, { spans: two }), run('a', 1)])
    const after = mergeSession(before, update)

    const next = followSelection(before, after, update, { run: { session: 'a', index: 0 }, spanId: 2 })

    expect(next).toEqual({ run: { session: 'a', index: 1 }, spanId: 1 })
  })

  test('the chosen span stays chosen while its run only grows', () => {
    const before = report([session('a', [run('a', 0, { spans: two })])])
    const update = session('a', [run('a', 0, { spans: [...two, span({ id: 3 })] })])
    const after = mergeSession(before, update)

    expect(followSelection(before, after, update, { run: { session: 'a', index: 0 }, spanId: 2 })).toEqual({ run: { session: 'a', index: 0 }, spanId: 2 })
    expect(followSelection(before, after, update, { run: { session: 'a', index: 0 }, spanId: 9 }).spanId).toBe(1)
  })

  test('someone reading an older run, or another session, is left where they are', () => {
    const before = report([session('a', [run('a', 0), run('a', 1)]), session('b', [run('b', 0)])])
    const update = session('a', [run('a', 0), run('a', 1), run('a', 2)])
    const after = mergeSession(before, update)

    const older = { run: { session: 'a', index: 0 }, spanId: 1 }
    const elsewhere = { run: { session: 'b', index: 0 }, spanId: 1 }
    expect(followSelection(before, after, update, older)).toBe(older)
    expect(followSelection(before, after, update, elsewhere)).toBe(elsewhere)
  })

  test('with nothing chosen yet the first run that arrives is chosen, and an empty update chooses nothing', () => {
    const before = report([])
    const update = session('a', [run('a', 0)])
    expect(followSelection(before, mergeSession(before, update), update, { run: null, spanId: 1 })).toEqual({ run: { session: 'a', index: 0 }, spanId: 1 })

    const empty = session('a', [])
    const nothing = { run: null, spanId: 1 }
    expect(followSelection(before, mergeSession(before, empty), empty, nothing)).toBe(nothing)
  })
})

describe('refetch', () => {
  const fresh = report([session('a', [run('a', 0)]), session('b', [run('b', 0)])])

  test('the new report keeps what each session last sent live, so a watched session keeps its own insights', async () => {
    const live = session('a', [run('a', 0), run('a', 1)], INSIGHTS)
    const { fetcher, asked } = answers({ '/api/report': { body: fresh }, '/api/latest': { body: [live] } })

    const merged = await refetch(fetcher)

    expect(asked).toEqual(['/api/report', '/api/latest'])
    expect(merged.sessions.map((entry) => [entry.id, entry.runs.length, entry.insights !== undefined])).toEqual([
      ['a', 2, true],
      ['b', 1, false],
    ])
  })

  test('a report that cannot be fetched or read is an error that says why', async () => {
    const down = answers({ '/api/report': { status: 503, body: {} } })
    await expect(refetch(down.fetcher)).rejects.toThrow('503')

    const garbled = answers({ '/api/report': { body: { ...fresh, sessions: 'none' } }, '/api/latest': { body: [] } })
    await expect(refetch(garbled.fetcher)).rejects.toThrow('report.sessions should be a list')
  })

  test('live updates that cannot be read are left out and the report still arrives', async () => {
    const good = session('b', [run('b', 0), run('b', 1)], INSIGHTS)
    const { fetcher } = answers({ '/api/report': { body: fresh }, '/api/latest': { body: [{ id: 'a', runs: 'x' }, good] } })
    const merged = await refetch(fetcher)
    expect(merged.sessions.map((entry) => entry.runs.length)).toEqual([1, 2])

    const without = answers({ '/api/report': { body: fresh }, '/api/latest': { status: 404, body: {} } })
    expect(await refetch(without.fetcher)).toEqual(fresh)
  })
})
