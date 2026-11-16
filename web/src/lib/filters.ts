import type { Run } from '@/types'
import { SIGNALS, SIGNAL_ORDER, STATUS_LABELS } from './signals'
import { shortId } from './format'

export type FacetKey = 'status' | 'model' | 'session' | 'signal'

export type Filters = Record<FacetKey, string[]>

export const EMPTY_FILTERS: Filters = { status: [], model: [], session: [], signal: [] }

export interface FacetOption {
  value: string
  label: string
}

export interface Facet {
  key: FacetKey
  title: string
  options: FacetOption[]
}

export function facetsFor(runs: Run[], current: string): Facet[] {
  const unique = (values: string[]) => [...new Set(values)]
  const statuses = unique(runs.map((run) => run.status))
  const models = unique(runs.map((run) => run.model)).sort()
  const sessions = unique(runs.map((run) => run.session))
  const signals = SIGNAL_ORDER.filter((signal) => runs.some((run) => run.signals.includes(signal)))
  return [
    { key: 'status', title: 'Status', options: statuses.map((value) => ({ value, label: STATUS_LABELS[value as Run['status']] ?? value })) },
    { key: 'model', title: 'Model', options: models.map((value) => ({ value, label: value })) },
    { key: 'session', title: 'Session', options: sessions.map((value) => ({ value, label: value === current ? `${shortId(value)} (current)` : shortId(value) })) },
    { key: 'signal', title: 'Signals', options: signals.map((value) => ({ value, label: SIGNALS[value].label })) },
  ]
}

export function matchesFacet(run: Run, key: FacetKey, value: string): boolean {
  if (key === 'status') return run.status === value
  if (key === 'model') return run.model === value
  if (key === 'session') return run.session === value
  return run.signals.includes(value as Run['signals'][number])
}

export function applyFilters(runs: Run[], filters: Filters, query: string): Run[] {
  const needle = query.trim().toLowerCase()
  return runs.filter(
    (run) =>
      (Object.keys(filters) as FacetKey[]).every((key) => !filters[key].length || filters[key].some((value) => matchesFacet(run, key, value))) &&
      (!needle || run.prompt.toLowerCase().includes(needle) || run.session.toLowerCase().includes(needle)),
  )
}

export function hasFilters(filters: Filters, query: string): boolean {
  return Boolean(query.trim()) || Object.values(filters).some((values) => values.length > 0)
}
