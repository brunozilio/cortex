import type { Day } from '@/types'
import { dayOf } from './format'

export type Metric = 'cost' | 'tokens' | 'runs'

export const PERIOD_DAYS = 30

export interface DayRow {
  day: number
  values: Record<string, number>
  total: number
}

function metricOf(entry: Day['models'][number], metric: Metric): number {
  if (metric === 'cost') return entry.cost_usd
  if (metric === 'tokens') return entry.tokens
  return entry.runs
}

export function modelsByCost(days: Day[]): string[] {
  const totals = new Map<string, number>()
  for (const day of days) for (const entry of day.models) totals.set(entry.model, (totals.get(entry.model) ?? 0) + entry.cost_usd)
  return [...totals.entries()].sort((a, b) => b[1] - a[1]).map(([model]) => model)
}

export function dailySeries(days: Day[], metric: Metric, generatedAtMs: number): DayRow[] {
  const byDay = new Map(days.map((day) => [day.day, day]))
  const last = dayOf(generatedAtMs)
  return Array.from({ length: PERIOD_DAYS }, (_, i) => {
    const day = last - PERIOD_DAYS + 1 + i
    const values: Record<string, number> = {}
    for (const entry of byDay.get(day)?.models ?? []) values[entry.model] = (values[entry.model] ?? 0) + metricOf(entry, metric)
    const total = Object.values(values).reduce((sum, value) => sum + value, 0)
    return { day, values, total }
  })
}

export function periodTotal(days: Day[], metric: Metric): number {
  return days.reduce((sum, day) => sum + day.models.reduce((acc, entry) => acc + metricOf(entry, metric), 0), 0)
}

export function weekOverWeek(series: DayRow[]): number | null {
  const last = series.slice(-7).reduce((sum, row) => sum + row.total, 0)
  const before = series.slice(-14, -7).reduce((sum, row) => sum + row.total, 0)
  if (!before) return null
  return (last - before) / before
}
