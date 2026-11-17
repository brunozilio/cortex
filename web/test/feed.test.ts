import { describe, expect, test } from 'bun:test'
import { feedNotice, feedStep, startFeed, type Feed, type FeedEvent } from '../src/lib/feed'
import { fixture, run, session } from './make'

function after(feed: Feed, ...events: FeedEvent[]): Feed {
  return events.reduce((current, event) => feedStep(current, event).feed, feed)
}

describe('the live feed', () => {
  test('a page without a terminal is static and says nothing', () => {
    const feed = startFeed(false)
    expect(feed).toEqual({ status: 'static', behind: false, problem: null })
    expect(feedNotice(feed)).toBeNull()
  })

  test('it is connecting until the stream opens, then live, with nothing to point out', () => {
    const connecting = startFeed(true)
    expect([connecting.status, feedNotice(connecting)]).toEqual(['connecting', null])

    const opened = feedStep(connecting, { type: 'open' })
    expect(opened).toEqual({ feed: { status: 'live', behind: false, problem: null }, effects: [] })
    expect(feedNotice(opened.feed)).toBeNull()
  })

  test('a lost connection is pointed out as disconnected until the stream is back', () => {
    const lost = after(startFeed(true), { type: 'open' }, { type: 'error' })
    expect(lost.status).toBe('reconnecting')
    expect(feedNotice(lost)).toContain('Disconnected')
    expect(feedNotice(lost)).toContain('Reconnecting')
  })

  test('coming back after a lost connection fetches everything again and says the data is partial meanwhile', () => {
    const lost = after(startFeed(true), { type: 'open' }, { type: 'error' })

    const back = feedStep(lost, { type: 'open' })

    expect(back.effects).toEqual([{ kind: 'resync' }])
    expect(back.feed).toEqual({ status: 'live', behind: true, problem: null })
    expect(feedNotice(back.feed)).toContain('partial')
    const caught = feedStep(back.feed, { type: 'caught_up' })
    expect(caught).toEqual({ feed: { status: 'live', behind: false, problem: null }, effects: [] })
    expect(feedNotice(caught.feed)).toBeNull()
  })

  test('a resync from the console, which dropped updates for this page, fetches everything again', () => {
    const live = after(startFeed(true), { type: 'open' })

    const told = feedStep(live, { type: 'resync' })

    expect(told.effects).toEqual([{ kind: 'resync' }])
    expect(told.feed.behind).toBe(true)
    expect(feedNotice(told.feed)).toContain('Some updates were missed')
  })

  test('a fetch that fails keeps the page marked partial and says why', () => {
    const behind = after(startFeed(true), { type: 'open' }, { type: 'resync' })
    const failed = feedStep(behind, { type: 'refetch_failed', message: 'the console answered 503' })
    expect(failed.feed.behind).toBe(true)
    expect(failed.effects).toEqual([])
    expect(feedNotice(failed.feed)).toContain('the console answered 503')
  })

  test('a new report is fetched without calling the page partial', () => {
    const live = after(startFeed(true), { type: 'open' })
    expect(feedStep(live, { type: 'report' })).toEqual({ feed: live, effects: [{ kind: 'report' }] })
  })

  test('a session update the page can read is handed on', () => {
    const live = after(startFeed(true), { type: 'open' })
    const update = session('a', [run('a', 0)])

    const step = feedStep(live, { type: 'message', data: JSON.stringify(update) })

    expect(step.effects).toEqual([{ kind: 'session', update }])
    expect(step.feed).toEqual(live)
  })

  test('the update the Rust side produces is one the page can read', () => {
    const update = fixture('session-update.json')
    const step = feedStep(startFeed(true), { type: 'message', data: JSON.stringify(update) })
    expect(step.feed.problem).toBeNull()
    expect(step.effects.map((effect) => effect.kind)).toEqual(['session'])
  })

  test('an update that is not JSON, or not a session, is left out and pointed out, until a good one arrives', () => {
    const live = after(startFeed(true), { type: 'open' })

    const garbled = feedStep(live, { type: 'message', data: '{"id":"a","ru' })
    expect(garbled.effects).toEqual([])
    expect(feedNotice(garbled.feed)).toContain('not JSON')

    const shapeless = feedStep(live, { type: 'message', data: JSON.stringify({ id: 'a', runs: [{ session: 'a' }] }) })
    expect(shapeless.effects).toEqual([])
    expect(feedNotice(shapeless.feed)).toContain('update.runs[0].index should be a number, not missing')

    const good = feedStep(shapeless.feed, { type: 'message', data: JSON.stringify(session('a', [])) })
    expect(good.feed.problem).toBeNull()
    expect(good.effects.length).toBe(1)
  })
})
