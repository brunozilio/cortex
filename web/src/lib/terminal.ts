export interface TerminalSize {
  cols: number
  rows: number
}

export function terminalUrl(location: Pick<Location, 'protocol' | 'host'>, session: string, size: TerminalSize): string {
  const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:'
  const query = new URLSearchParams({ session, cols: String(size.cols), rows: String(size.rows) })
  return `${scheme}//${location.host}/api/terminal?${query}`
}

export function resizeMessage(size: TerminalSize): string {
  return JSON.stringify({ cols: size.cols, rows: size.rows })
}
