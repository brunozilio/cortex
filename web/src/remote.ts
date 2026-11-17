import { installTunnel, onRelay, type RemoteState } from './lib/tunnel/install'

const BAR_ID = 'remote-status'

function words(state: RemoteState): { text: string; tone: 'quiet' | 'warn' } {
  switch (state.kind) {
    case 'connecting':
      return { text: state.name ? `Connecting as ${state.name}…` : 'Connecting…', tone: 'quiet' }
    case 'connected':
      return { text: `Connected as ${state.name}`, tone: 'quiet' }
    case 'unpaired':
      return { text: 'This page is not paired. On the machine run `remote pair` and open the link it prints.', tone: 'warn' }
    case 'refused':
      return { text: `Not let in: ${state.reason}. Pair again from the machine.`, tone: 'warn' }
    case 'down':
      return { text: `Connection lost (${state.reason}); retrying on the next request.`, tone: 'warn' }
  }
}

function show(state: RemoteState) {
  const { text, tone } = words(state)
  let bar = document.getElementById(BAR_ID)
  if (!bar) {
    bar = document.createElement('div')
    bar.id = BAR_ID
    bar.setAttribute('role', 'status')
    document.body.appendChild(bar)
  }
  bar.className = [
    'fixed inset-x-0 bottom-0 z-50 px-4 py-2 text-xs',
    'pb-[max(0.5rem,env(safe-area-inset-bottom))]',
    tone === 'warn' ? 'bg-amber-500 text-black' : 'bg-zinc-900/90 text-zinc-100',
  ].join(' ')
  bar.textContent = text
  if (state.kind === 'connected') setTimeout(() => bar?.remove(), 2_500)
}

if (onRelay(document)) {
  installTunnel((state) => (document.body ? show(state) : addEventListener('DOMContentLoaded', () => show(state), { once: true })))
  if ('serviceWorker' in navigator) void navigator.serviceWorker.register('/sw.js').catch(() => undefined)
}
