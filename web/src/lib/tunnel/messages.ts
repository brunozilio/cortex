export type Payload = { kind: 'text'; value: string } | { kind: 'bytes'; value: string }

export type Ask =
  | { type: 'fetch'; id: number; method: string; path: string; body?: string; content_type?: string }
  | { type: 'events'; id: number; path: string }
  | { type: 'socket'; id: number; path: string }
  | { type: 'send'; id: number; data: Payload }
  | { type: 'close'; id: number }

export type Answer =
  | { type: 'head'; id: number; status: number; content_type?: string }
  | { type: 'data'; id: number; data: Payload }
  | { type: 'end'; id: number; error?: string }

export interface AnswerChannel {
  send(ask: Ask): void
  onAnswer: (answer: Answer) => void
  onDown: (reason: string) => void
}
