export interface TerminalSize {
  cols: number
  rows: number
}

const LARGEST = 65535

export function terminalUrl(location: Pick<Location, 'protocol' | 'host'>, session: string, size: TerminalSize): string {
  const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const query = new URLSearchParams({ session, cols: String(size.cols), rows: String(size.rows) })
  return `${scheme}//${location.host}/api/terminal?${query}`
}

function taken(cells: number): boolean {
  return Number.isInteger(cells) && cells >= 1 && cells <= LARGEST
}

export function resizeMessage(size: TerminalSize): string | null {
  if (!taken(size.cols) || !taken(size.rows)) return null
  return JSON.stringify({ cols: size.cols, rows: size.rows })
}

export function screenSize(message: string): TerminalSize | null {
  let read: unknown
  try {
    read = JSON.parse(message)
  } catch {
    return null
  }
  if (typeof read !== 'object' || read === null) return null
  const { cols, rows } = read as Record<string, unknown>
  if (typeof cols !== 'number' || typeof rows !== 'number' || !taken(cols) || !taken(rows)) return null
  return { cols, rows }
}
