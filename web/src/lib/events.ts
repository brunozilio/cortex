import type { Span } from '@/types'

const TITLES = new Map([
  ['notice', 'Notice'],
  ['notified', 'Inbox notice'],
  ['partial', 'Partial trace'],
  ['diagnosed', 'Diagnosis'],
  ['mode', 'Mode changed'],
  ['moved', 'Folder moved'],
  ['asked', 'Question'],
  ['compacted', 'Context compacted'],
  ['rewound', 'Rewound'],
  ['steered', 'Steered'],
  ['failed', 'Failed'],
  ['cancelled', 'Cancelled'],
])

const FELL_BACK = 'fell back to '

export function eventTitle(span: Span): string {
  if (span.kind !== 'event') return span.name
  if (span.name.startsWith(FELL_BACK)) return `Fell back to ${span.name.slice(FELL_BACK.length)}`
  return TITLES.get(span.name) ?? span.name
}

export function eventText(span: Span): string {
  return span.output || span.input || span.name
}
