import { describe, expect, test } from 'bun:test'
import { loadReport, scoped } from '../src/lib/report'
import type { Report } from '../src/types'
import { INSIGHTS, answers, fixture } from './make'

const REPORT: Report = { project: 'real', current: 's1', generated_at_ms: 1, sessions: [{ id: 's1', runs: [] }], insights: INSIGHTS }
const SAMPLE: Report = { ...REPORT, project: 'sample' }

function answering(...answers: (number | Error)[]): { fetcher: typeof fetch; asked: () => number } {
  let asked = 0
  const fetcher = (async () => {
    const answer = answers[Math.min(asked, answers.length - 1)]
    asked += 1
    if (answer instanceof Error) throw answer
    return new Response(answer === 200 ? JSON.stringify(REPORT) : '{}', { status: answer })
  }) as unknown as typeof fetch
  return { fetcher, asked: () => asked }
}

describe('loadReport', () => {
  test('a page without access says to open it from the terminal link, with no sample data', async () => {
    const { fetcher, asked } = answering(401)
    const loaded = await loadReport(fetcher, 0)
    expect(loaded.kind).toBe('failed')
    if (loaded.kind !== 'failed') return
    expect(loaded.status).toBe(401)
    expect(loaded.message).toContain('link the terminal printed')
    expect(asked()).toBe(1)
  })

  test('a report that is not ready yet is asked for again until it arrives', async () => {
    const { fetcher, asked } = answering(503, 503, 200)
    const loaded = await loadReport(fetcher, 0)
    expect(loaded).toEqual({ kind: 'live', report: REPORT })
    expect(asked()).toBe(3)
  })

  test('a report that never becomes ready ends as a failure, not as sample data', async () => {
    const { fetcher } = answering(503)
    const loaded = await loadReport(fetcher, 0)
    expect(loaded.kind).toBe('failed')
    if (loaded.kind === 'failed') expect(loaded.message).toContain('has not sent its report')
  })

  test('a terminal that does not answer is reported as closed', async () => {
    const { fetcher, asked } = answering(new TypeError('fetch failed'))
    const loaded = await loadReport(fetcher, 0)
    expect(loaded.kind).toBe('failed')
    if (loaded.kind !== 'failed') return
    expect(loaded.status).toBeNull()
    expect(loaded.message).toContain('not answering')
    expect(asked()).toBe(1)
  })

  test('sample data shows only when the caller offers it, as development does', async () => {
    const { fetcher } = answering(new TypeError('fetch failed'))
    const loaded = await loadReport(fetcher, 0, async () => ({ report: SAMPLE, insights: () => INSIGHTS }))
    expect(loaded.kind).toBe('sample')
    if (loaded.kind === 'sample') expect(loaded.report.project).toBe('sample')
  })
})

describe('scoped', () => {
  test('a live session without its own insights waits for them instead of inventing some', () => {
    expect(scoped(REPORT, 's1').ready).toBe(false)
    expect(scoped(REPORT, 's1', () => INSIGHTS).ready).toBe(true)
  })
})

describe('a report the page cannot read', () => {
  test('is a failure that says which field is wrong, not a page drawn from it', async () => {
    const { fetcher, asked } = answers({ '/api/report': { body: { ...REPORT, sessions: [{ id: 's1', runs: [{ session: 's1' }] }] } } })

    const loaded = await loadReport(fetcher, 0)

    expect(loaded.kind).toBe('failed')
    if (loaded.kind !== 'failed') return
    expect(loaded.message).toContain('report.sessions[0].runs[0].index should be a number, not missing')
    expect(loaded.message).toContain('different versions')
    expect(asked.length).toBe(1)
  })

  test('is not replaced by sample data, even in development', async () => {
    const { fetcher } = answers({ '/api/report': { body: [] } })
    const loaded = await loadReport(fetcher, 0, async () => ({ report: SAMPLE, insights: () => INSIGHTS }))
    expect(loaded.kind).toBe('failed')
  })

  test('while the report the Rust side serializes loads', async () => {
    const report = fixture('report.json')
    const { fetcher } = answers({ '/api/report': { body: report } })
    expect(await loadReport(fetcher, 0)).toEqual({ kind: 'live', report: report as Report })
  })
})
