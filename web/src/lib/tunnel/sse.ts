export interface ServerEvent {
  event: string
  data: string
}

export class EventParser {
  private buffer = ''
  private name = ''
  private data: string[] = []

  push(text: string): ServerEvent[] {
    this.buffer += text
    const events: ServerEvent[] = []
    let newline = this.buffer.search(/\r\n|\r|\n/)
    while (newline >= 0) {
      const line = this.buffer.slice(0, newline)
      const width = this.buffer.startsWith('\r\n', newline) ? 2 : 1
      this.buffer = this.buffer.slice(newline + width)
      const event = this.line(line)
      if (event) events.push(event)
      newline = this.buffer.search(/\r\n|\r|\n/)
    }
    return events
  }

  private line(line: string): ServerEvent | null {
    if (line === '') {
      const event = this.data.length > 0 ? { event: this.name || 'message', data: this.data.join('\n') } : null
      this.name = ''
      this.data = []
      return event
    }
    if (line.startsWith(':')) return null
    const colon = line.indexOf(':')
    const field = colon < 0 ? line : line.slice(0, colon)
    const value = colon < 0 ? '' : line.slice(colon + 1).replace(/^ /, '')
    if (field === 'event') this.name = value
    if (field === 'data') this.data.push(value)
    return null
  }
}
