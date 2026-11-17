import type { Span } from '@/types'
import { EN, type Strings } from './i18n'

const FELL_BACK = 'fell back to '

export function eventTitle(span: Span, strings: Strings = EN): string {
  if (span.kind !== 'event') return span.name
  if (span.name.startsWith(FELL_BACK)) return strings.fellBack(span.name.slice(FELL_BACK.length))
  return Object.hasOwn(strings.events, span.name) ? strings.events[span.name] : span.name
}

export function eventText(span: Span): string {
  return span.output || span.input || span.name
}
