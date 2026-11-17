import { EN, type Strings } from './i18n'

const DAY_MS = 86_400_000

export function usd(value: number): string {
  if (value === 0) return '$0.00'
  if (value < 0.01) return '$' + value.toFixed(4)
  return '$' + value.toFixed(value < 1 ? 3 : 2)
}

export function usdShort(value: number): string {
  return '$' + value.toFixed(2)
}

export function ms(value: number): string {
  if (value < 1000) return Math.round(value) + 'ms'
  if (value < 10_000) return (value / 1000).toFixed(1) + 's'
  if (value < 59_500) return Math.round(value / 1000) + 's'
  const totalSeconds = Math.round(value / 1000)
  if (totalSeconds < 3600) {
    return `${Math.floor(totalSeconds / 60)}m ${String(totalSeconds % 60).padStart(2, '0')}s`
  }
  const totalMinutes = Math.round(value / 60_000)
  return `${Math.floor(totalMinutes / 60)}h ${String(totalMinutes % 60).padStart(2, '0')}m`
}

export function tokens(value: number): string {
  if (value >= 1e6) return (value / 1e6).toFixed(1) + 'M'
  if (value >= 1000) return (value / 1000).toFixed(1) + 'k'
  return String(Math.round(value))
}

export function percent(ratio: number): string {
  return Math.round(ratio * 100) + '%'
}

export function dayLabel(day: number, locale?: string): string {
  return new Date(day * DAY_MS).toLocaleDateString(locale, { month: 'short', day: 'numeric', timeZone: 'UTC' })
}

export function dayOf(epochMs: number): number {
  return Math.floor(epochMs / DAY_MS)
}

function localDayStart(epochMs: number): number {
  const date = new Date(epochMs)
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()
}

export function when(epochMs: number, nowMs: number, strings: Strings = EN): string {
  const time = new Date(epochMs).toLocaleTimeString(strings.locale, { hour: '2-digit', minute: '2-digit' })
  const daysAgo = Math.round((localDayStart(nowMs) - localDayStart(epochMs)) / DAY_MS)
  if (daysAgo === 0) return `${strings.today} ${time}`
  if (daysAgo === 1) return `${strings.yesterday} ${time}`
  const date = new Date(epochMs).toLocaleDateString(strings.locale, { month: 'short', day: 'numeric' })
  return `${date} ${time}`
}

export function shortId(id: string): string {
  return id.length > 8 ? id.slice(0, 8) : id
}
