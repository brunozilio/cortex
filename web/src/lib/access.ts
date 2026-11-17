export const ACCESS_KEY = 'console-access'
export const ENTER_PATH = '/api/enter'
export const ENTRY_HEADER = 'x-console-entry'
export const ACCESS_QUERY = 'access_token'

export type KeptToken = { get(): string | null; set(token: string): void }

type Place = { pathname: string; search: string; hash: string; host: string; protocol: string }
type History = { replaceState(data: unknown, unused: string, url: string): void }

export function storedIn(storage: () => Storage | undefined): KeptToken {
  return {
    get() {
      try {
        return storage()?.getItem(ACCESS_KEY) ?? null
      } catch {
        return null
      }
    },
    set(token) {
      try {
        storage()?.setItem(ACCESS_KEY, token)
      } catch {
        return
      }
    },
  }
}

export function entryCode(hash: string): string | null {
  const params = new URLSearchParams(hash.startsWith('#') ? hash.slice(1) : hash)
  const code = params.get('enter')
  return code && /^[0-9a-f]{16,128}$/.test(code) ? code : null
}

export async function enter(place: Place, history: History, kept: KeptToken, fetcher: typeof fetch): Promise<boolean> {
  const code = entryCode(place.hash)
  if (!code) return false
  history.replaceState(null, '', place.pathname + place.search)
  const response = await fetcher(ENTER_PATH, { method: 'POST', headers: { [ENTRY_HEADER]: code } }).catch(() => null)
  if (!response?.ok) return false
  const answer = (await response.json().catch(() => null)) as { token?: unknown } | null
  if (typeof answer?.token !== 'string') return false
  kept.set(answer.token)
  return true
}

function sameServerApi(url: string | URL, place: Place): URL | null {
  let parsed: URL
  try {
    parsed = new URL(String(url), `${place.protocol}//${place.host}/`)
  } catch {
    return null
  }
  const local = ['http:', 'https:', 'ws:', 'wss:'].includes(parsed.protocol) && parsed.host === place.host
  return local && parsed.pathname.startsWith('/api/') ? parsed : null
}

export function withToken(url: string | URL, place: Place, token: string | null): string {
  const api = sameServerApi(url, place)
  if (!api || !token || api.pathname === ENTER_PATH) return String(url)
  api.searchParams.set(ACCESS_QUERY, token)
  return api.toString()
}

export function authorized(
  input: RequestInfo | URL,
  init: RequestInit | undefined,
  place: Place,
  token: string | null,
): [RequestInfo | URL, RequestInit | undefined] {
  const url = typeof input === 'string' || input instanceof URL ? input : input.url
  const api = sameServerApi(url, place)
  if (!api || !token || api.pathname === ENTER_PATH) return [input, init]
  const headers = new Headers(init?.headers ?? (input instanceof Request ? input.headers : undefined))
  if (!headers.has('authorization')) headers.set('authorization', `Bearer ${token}`)
  return [input, { ...init, headers }]
}

export function installAccess(scope: Record<string, unknown>, place: Place, kept: KeptToken): void {
  const fetcher = scope.fetch as typeof fetch
  scope.fetch = (input: RequestInfo | URL, init?: RequestInit) => fetcher(...authorized(input, init, place, kept.get()))
  const Events = scope.EventSource as (new (url: string | URL, init?: EventSourceInit) => EventSource) | undefined
  if (Events) {
    scope.EventSource = Object.assign(
      function AuthorizedEventSource(url: string | URL, init?: EventSourceInit) {
        return new Events(withToken(url, place, kept.get()), init)
      },
      { CONNECTING: 0, OPEN: 1, CLOSED: 2 },
    )
  }
  const Socket = scope.WebSocket as (new (url: string | URL, protocols?: string | string[]) => WebSocket) | undefined
  if (Socket) {
    scope.WebSocket = Object.assign(
      function AuthorizedWebSocket(url: string | URL, protocols?: string | string[]) {
        return new Socket(withToken(url, place, kept.get()), protocols)
      },
      { CONNECTING: 0, OPEN: 1, CLOSING: 2, CLOSED: 3 },
    )
  }
}

export async function prepareAccess(): Promise<void> {
  const kept = storedIn(() => globalThis.localStorage)
  const scope = globalThis as unknown as Record<string, unknown>
  const original = (scope.fetch as typeof fetch).bind(globalThis)
  installAccess(scope, window.location, kept)
  await enter(window.location, window.history, kept, original)
}
