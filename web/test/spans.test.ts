import { describe, expect, test } from 'bun:test'
import {
  attr,
  callCacheRatio,
  findingsFor,
  isRepeated,
  modelCalls,
  priciestCall,
  promptTokens,
  refusalReason,
  rootSpan,
  runCacheRatio,
  spanById,
  timelineEnd,
  treeOrder,
} from '../src/lib/spans'
import type { Finding, Usage } from '../src/types'
import { run, span } from './make'

const usage = (fields: Partial<Usage>): Usage => ({ model: 'm', input: 0, output: 0, cache_read: 0, cache_write: 0, cost_usd: 0, ...fields })

describe('treeOrder', () => {
  test('children follow their parent, in the order they started, one level deeper', () => {
    const spans = [
      span({ id: 1, parent: 0, kind: 'run' }),
      span({ id: 2, parent: 1, start_ms: 50 }),
      span({ id: 3, parent: 1, start_ms: 10 }),
      span({ id: 4, parent: 3, start_ms: 20 }),
      span({ id: 5, parent: 1, start_ms: 10 }),
    ]
    expect(treeOrder(spans).map(({ span: placed, depth }) => [placed.id, depth])).toEqual([
      [1, 0],
      [3, 1],
      [4, 2],
      [5, 1],
      [2, 1],
    ])
  })

  test('a span whose parent is missing is still shown, with its own children under it', () => {
    const spans = [span({ id: 1, parent: 0, kind: 'run' }), span({ id: 7, parent: 99 }), span({ id: 8, parent: 7 }), span({ id: 9, parent: 8 })]
    expect(treeOrder(spans).map(({ span: placed, depth }) => [placed.id, depth])).toEqual([
      [1, 0],
      [7, 1],
      [8, 2],
      [9, 3],
    ])
  })

  test('spans that name each other as parent are shown once and do not loop', () => {
    const spans = [span({ id: 1, parent: 0, kind: 'run' }), span({ id: 2, parent: 3 }), span({ id: 3, parent: 2 }), span({ id: 4, parent: 4 })]
    const placed = treeOrder(spans)
    expect(placed.map((entry) => entry.span.id).sort()).toEqual([1, 2, 3, 4])
  })
})

describe('a run', () => {
  const llm = span({ id: 2, kind: 'llm', end_ms: 400, usage: usage({ input: 100, cache_read: 300, cache_write: 100, output: 20, cost_usd: 0.01 }) })
  const tool = span({ id: 3, kind: 'tool', end_ms: 900, usage: usage({ cost_usd: 0.5 }) })
  const subject = run('s', 0, { duration_ms: 500, input: 1000, cache_read: 250, spans: [span({ id: 1, parent: 0, kind: 'run' }), llm, tool, span({ id: 4, kind: 'llm' })] })

  test('its root is the span without a parent, else its first span', () => {
    expect(rootSpan(subject)?.id).toBe(1)
    expect(rootSpan(run('s', 0, { spans: [span({ id: 5, parent: 2 })] }))?.id).toBe(5)
    expect(rootSpan(run('s', 0, { spans: [] }))).toBeUndefined()
  })

  test('its timeline ends at its latest span, never before its duration and never at zero', () => {
    expect(timelineEnd(subject)).toBe(900)
    expect(timelineEnd(run('s', 0, { duration_ms: 2_000, spans: [llm] }))).toBe(2_000)
    expect(timelineEnd(run('s', 0, { duration_ms: 0, spans: [] }))).toBe(1)
  })

  test('spans are found by id', () => {
    expect(spanById(subject, 3)).toBe(tool)
    expect(spanById(subject, 42)).toBeUndefined()
  })

  test('prompt tokens count what was read from and written to the cache', () => {
    expect(promptTokens(llm.usage!)).toBe(500)
    expect(callCacheRatio(llm.usage!)).toBe(0.6)
    expect(callCacheRatio(usage({}))).toBe(0)
    expect(runCacheRatio(subject)).toBe(0.25)
    expect(runCacheRatio(run('s', 0))).toBe(0)
  })

  test('model calls are the model spans that were billed, and the priciest call may be a tool', () => {
    expect(modelCalls(subject).map((call) => call.id)).toEqual([2])
    expect(priciestCall(subject)?.id).toBe(3)
    expect(priciestCall(run('s', 0))).toBeUndefined()
  })
})

describe('a span', () => {
  test('attributes are read by name', () => {
    const subject = span({ id: 2, attrs: [['tool', 'Bash'], ['call', 'b1']] })
    expect(attr(subject, 'call')).toBe('b1')
    expect(attr(subject, 'nope')).toBeUndefined()
  })

  test('flags say when it was repeated or refused, and why', () => {
    expect(isRepeated(span({ id: 2, flags: ['edited a.rs', 'repeated'] }))).toBe(true)
    expect(isRepeated(span({ id: 2, flags: ['refused: no'] }))).toBe(false)
    expect(refusalReason(span({ id: 2, flags: ['refused: outside the project'] }))).toBe('outside the project')
    expect(refusalReason(span({ id: 2, flags: ['refused'] }))).toBe('refused')
    expect(refusalReason(span({ id: 2, flags: ['repeated'] }))).toBeUndefined()
  })
})

describe('findingsFor', () => {
  const finding = (fields: Partial<Finding>): Finding => ({ session: 's', run: 0, span: 2, check: 'review', text: 't', at_ms: 0, severe: false, ...fields })
  const all = [finding({ span: 2 }), finding({ span: 3 }), finding({ run: 1 }), finding({ session: 'other' })]
  const subject = run('s', 0)

  test('a run gets its own findings, its root span all of them and any other span only its own', () => {
    expect(findingsFor(all, subject).map((found) => found.span)).toEqual([2, 3])
    expect(findingsFor(all, subject, span({ id: 1, parent: 0 })).length).toBe(2)
    expect(findingsFor(all, subject, span({ id: 3 })).map((found) => found.span)).toEqual([3])
    expect(findingsFor(all, subject, span({ id: 9 }))).toEqual([])
  })
})
