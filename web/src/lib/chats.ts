export type ChatStatus = 'idle' | 'running' | 'thinking' | 'needs_you' | 'done' | 'failed' | 'cancelled'

export interface ChatCard {
  session: string
  title: string | null
  status: ChatStatus
  updated: number
  folder: string
}

export interface Entry {
  kind: string
  data?: unknown
}

export type AskKind =
  | { Permission: { tool: string; subject: string; rememberable: boolean; rule?: string } }
  | { Plan: { plan: string } }
  | { Questions: unknown[] }

export interface Ask {
  id: string
  kind: AskKind
}

export interface Chat {
  session: string
  title: string | null
  status: ChatStatus
  entries: Entry[]
  ask: Ask | null
}

export interface Step {
  name: string
  summary: string
  state: 'running' | 'done' | 'failed'
}

export type Bubble =
  | { kind: 'mine'; text: string }
  | { kind: 'theirs'; text: string; label?: string }
  | { kind: 'steps'; steps: Step[] }
  | { kind: 'note'; text: string; tone: 'quiet' | 'bad' }

export type Decision = 'allow' | 'always' | 'deny' | 'approve'

export interface Choice {
  label: string
  decision: Decision
  tone: 'go' | 'stop'
}

export interface AskView {
  title: string
  detail: string
  choices: Choice[]
}

export interface ChatWords {
  chats: string
  newChat: string
  empty: string
  emptyChat: string
  placeholder: string
  working: string
  waiting: string
  stopped: string
  failed: string
  stop: string
  trace: string
  allow: string
  always: string
  deny: string
  approve: string
  keepPlanning: string
  plan: string
  askTool: (tool: string) => string
  askPlan: string
  askQuestions: string
  answerOnMachine: string
  skip: string
  refused: (tool: string, reason: string) => string
  interrupted: string
  agentStep: (kind: string) => string
  steps: (count: number) => string
  untitled: string
  now: string
  minutes: (count: number) => string
  hours: (count: number) => string
  days: (count: number) => string
  offline: string
}

export const CHAT_WORDS: Record<'en' | 'pt', ChatWords> = {
  en: {
    chats: 'Chats',
    newChat: 'New chat',
    empty: 'No chats yet. Start one with the button below.',
    emptyChat: 'Say what to do. The agent works on the machine, in its project folder.',
    placeholder: 'Message',
    working: 'working…',
    waiting: 'waiting for you',
    stopped: 'stopped',
    failed: 'failed',
    stop: 'Stop',
    trace: 'Trace',
    allow: 'Allow',
    always: 'Always allow',
    deny: 'Deny',
    approve: 'Approve plan',
    keepPlanning: 'Keep planning',
    plan: 'Plan',
    askTool: (tool) => `Allow ${tool}?`,
    askPlan: 'Carry out this plan?',
    askQuestions: 'The agent has questions',
    answerOnMachine: 'Answer them on the machine, or skip them here.',
    skip: 'Skip',
    refused: (tool, reason) => `${tool} was refused: ${reason}`,
    interrupted: 'Stopped',
    agentStep: (kind) => `agent ${kind}`,
    steps: (count) => (count === 1 ? '1 step' : `${count} steps`),
    untitled: 'New chat',
    now: 'now',
    minutes: (count) => `${count} min`,
    hours: (count) => `${count} h`,
    days: (count) => `${count} d`,
    offline: 'Could not reach the machine. Is a chat open there?',
  },
  pt: {
    chats: 'Conversas',
    newChat: 'Nova conversa',
    empty: 'Nenhuma conversa ainda. Comece uma no botão abaixo.',
    emptyChat: 'Diga o que fazer. O agente trabalha na máquina, na pasta do projeto.',
    placeholder: 'Mensagem',
    working: 'trabalhando…',
    waiting: 'esperando você',
    stopped: 'parado',
    failed: 'falhou',
    stop: 'Parar',
    trace: 'Trace',
    allow: 'Permitir',
    always: 'Sempre permitir',
    deny: 'Negar',
    approve: 'Aprovar plano',
    keepPlanning: 'Continuar planejando',
    plan: 'Plano',
    askTool: (tool) => `Permitir ${tool}?`,
    askPlan: 'Executar este plano?',
    askQuestions: 'O agente tem perguntas',
    answerOnMachine: 'Responda na máquina, ou pule aqui.',
    skip: 'Pular',
    refused: (tool, reason) => `${tool} foi recusado: ${reason}`,
    interrupted: 'Parado',
    agentStep: (kind) => `agente ${kind}`,
    steps: (count) => (count === 1 ? '1 passo' : `${count} passos`),
    untitled: 'Nova conversa',
    now: 'agora',
    minutes: (count) => `${count} min`,
    hours: (count) => `${count} h`,
    days: (count) => `${count} d`,
    offline: 'Não deu para falar com a máquina. Há um chat aberto nela?',
  },
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : ''
}

function field(value: unknown, name: string): unknown {
  return value && typeof value === 'object' ? (value as Record<string, unknown>)[name] : undefined
}

function step(entry: Entry, words: ChatWords): Step | null {
  if (entry.kind === 'tool') {
    const state = text(field(entry.data, 'state'))
    return {
      name: text(field(entry.data, 'name')),
      summary: text(field(entry.data, 'summary')),
      state: state === 'failed' ? 'failed' : state === 'done' ? 'done' : 'running',
    }
  }
  if (entry.kind === 'agent') {
    const done = field(entry.data, 'done')
    return {
      name: words.agentStep(text(field(entry.data, 'kind'))),
      summary: text(field(entry.data, 'summary')),
      state: done === false ? 'failed' : done === true ? 'done' : 'running',
    }
  }
  return null
}

function single(entry: Entry, words: ChatWords): Bubble | null {
  switch (entry.kind) {
    case 'user':
      return { kind: 'mine', text: text(entry.data) }
    case 'assistant':
      return text(entry.data).trim() ? { kind: 'theirs', text: text(entry.data) } : null
    case 'plan':
      return { kind: 'theirs', text: text(field(entry.data, 'plan')), label: words.plan }
    case 'refused':
      return { kind: 'note', text: words.refused(text(field(entry.data, 'tool')), text(field(entry.data, 'reason'))), tone: 'bad' }
    case 'error':
      return { kind: 'note', text: text(entry.data), tone: 'bad' }
    case 'interrupted':
      return { kind: 'note', text: words.interrupted, tone: 'quiet' }
    case 'heads_up':
      return { kind: 'note', text: text(field(entry.data, 'title')), tone: field(entry.data, 'urgent') ? 'bad' : 'quiet' }
    case 'notice':
    case 'recap':
      return text(entry.data).trim() ? { kind: 'note', text: text(entry.data), tone: 'quiet' } : null
    case 'held':
      return { kind: 'note', text: text(field(entry.data, 'text')), tone: 'quiet' }
    default:
      return null
  }
}

export function bubbles(entries: Entry[], words: ChatWords): Bubble[] {
  const shown: Bubble[] = []
  for (const entry of entries) {
    const found = step(entry, words)
    const last = shown[shown.length - 1]
    if (found && last?.kind === 'steps') {
      last.steps.push(found)
      continue
    }
    if (found) {
      shown.push({ kind: 'steps', steps: [found] })
      continue
    }
    const bubble = single(entry, words)
    if (bubble) shown.push(bubble)
  }
  return shown
}

export function askView(ask: Ask, words: ChatWords): AskView {
  const kind = ask.kind
  if ('Permission' in kind) {
    const { tool, subject, rememberable } = kind.Permission
    const choices: Choice[] = [{ label: words.allow, decision: 'allow', tone: 'go' }]
    if (rememberable) choices.push({ label: words.always, decision: 'always', tone: 'go' })
    choices.push({ label: words.deny, decision: 'deny', tone: 'stop' })
    return { title: words.askTool(tool), detail: subject, choices }
  }
  if ('Plan' in kind) {
    return {
      title: words.askPlan,
      detail: kind.Plan.plan,
      choices: [
        { label: words.approve, decision: 'approve', tone: 'go' },
        { label: words.keepPlanning, decision: 'deny', tone: 'stop' },
      ],
    }
  }
  return { title: words.askQuestions, detail: words.answerOnMachine, choices: [{ label: words.skip, decision: 'deny', tone: 'stop' }] }
}

export function busy(status: ChatStatus): boolean {
  return status === 'running' || status === 'thinking' || status === 'needs_you'
}

export function ago(updatedMs: number, nowMs: number, words: ChatWords): string {
  const minutes = Math.floor(Math.max(0, nowMs - updatedMs) / 60_000)
  if (minutes < 1) return words.now
  if (minutes < 60) return words.minutes(minutes)
  if (minutes < 60 * 24) return words.hours(Math.floor(minutes / 60))
  return words.days(Math.floor(minutes / (60 * 24)))
}

export function folderName(folder: string): string {
  const parts = folder.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] ?? folder
}

export function initials(name: string): string {
  const words = name.split(/[\s_\-.]+/).filter(Boolean)
  const letters = words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? '?').slice(0, 2)
  return letters.toUpperCase()
}

async function json<T>(response: Response): Promise<T> {
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: { message?: string } } | null
    throw new Error(body?.error?.message ?? `HTTP ${response.status}`)
  }
  return (await response.json()) as T
}

function posted(body: unknown): RequestInit {
  return { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) }
}

export interface ChatApi {
  list(): Promise<ChatCard[]>
  open(session: string): Promise<Chat>
  start(text: string): Promise<string>
  send(session: string, text: string): Promise<void>
  answer(session: string, ask: string, decision: Decision): Promise<void>
  stop(session: string): Promise<void>
}

export function chatApi(fetcher: typeof fetch): ChatApi {
  const at = (session: string) => `/api/chats/${encodeURIComponent(session)}`
  return {
    list: async () => json<ChatCard[]>(await fetcher('/api/chats')),
    open: async (session) => json<Chat>(await fetcher(at(session))),
    start: async (text) => (await json<{ session: string }>(await fetcher('/api/chats', posted({ text })))).session,
    send: async (session, text) => void (await json(await fetcher(`${at(session)}/messages`, posted({ text })))),
    answer: async (session, ask, decision) => void (await json(await fetcher(`${at(session)}/answer`, posted({ ask, decision })))),
    stop: async (session) => void (await json(await fetcher(`${at(session)}/stop`, posted({})))),
  }
}

export type ChatPlace = { screen: 'list' } | { screen: 'new' } | { screen: 'chat'; session: string }

export function placeOf(pathname: string): ChatPlace | null {
  if (pathname === '/chat' || pathname === '/chat/') return { screen: 'list' }
  if (pathname === '/chat/new') return { screen: 'new' }
  const match = /^\/chat\/([A-Za-z0-9_-]+)\/?$/.exec(pathname)
  return match ? { screen: 'chat', session: match[1] } : null
}

export function pathOf(place: ChatPlace): string {
  if (place.screen === 'list') return '/chat'
  if (place.screen === 'new') return '/chat/new'
  return `/chat/${place.session}`
}
