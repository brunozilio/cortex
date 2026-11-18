import { installTunnel, onRelay, type RemoteState } from './lib/tunnel/install'
import { pastedPairing } from './lib/tunnel/link'

const BAR_ID = 'remote-status'

const PASTE = navigator.language.toLowerCase().startsWith('pt')
  ? {
      hint: 'Cole aqui o link de pareamento',
      pair: 'Parear',
      wrong: 'Isso não é um link de pareamento.',
      unpaired: 'Esta página não está pareada. Na máquina rode `remote pair` e abra o link, ou cole-o abaixo.',
      installed:
        'Este app da tela de início ainda não está pareado: ele guarda o pareamento à parte do navegador. Na máquina rode `remote pair` e cole o link abaixo; depois disso ele fica pareado.',
    }
  : {
      hint: 'Paste the pairing link here',
      pair: 'Pair',
      wrong: 'That is not a pairing link.',
      unpaired: 'This page is not paired. On the machine run `remote pair`, then open the link it prints or paste it below.',
      installed:
        'This home-screen app is not paired yet: it keeps its pairing apart from the browser. On the machine run `remote pair` and paste the link below; it stays paired after that.',
    }

function words(state: RemoteState): { text: string; tone: 'quiet' | 'warn' } {
  switch (state.kind) {
    case 'connecting':
      return { text: state.name ? `Connecting as ${state.name}…` : 'Connecting…', tone: 'quiet' }
    case 'connected':
      return { text: `Connected as ${state.name}`, tone: 'quiet' }
    case 'unpaired':
      return { text: installed() ? PASTE.installed : PASTE.unpaired, tone: 'warn' }
    case 'refused':
      return { text: `Not let in: ${state.reason}. Pair again from the machine.`, tone: 'warn' }
    case 'down':
      return { text: `Connection lost (${state.reason}); retrying on the next request.`, tone: 'warn' }
  }
}

function installed(): boolean {
  const standalone = (navigator as Navigator & { standalone?: boolean }).standalone === true
  return standalone || window.matchMedia?.('(display-mode: standalone)').matches === true
}

function pasteForm(bar: HTMLElement) {
  const form = document.createElement('form')
  form.className = 'mt-2 flex gap-2'
  const input = document.createElement('input')
  input.type = 'url'
  input.placeholder = PASTE.hint
  input.autocomplete = 'off'
  input.className = 'min-w-0 flex-1 rounded-md bg-white px-2 py-1.5 text-[16px] text-black'
  const button = document.createElement('button')
  button.type = 'submit'
  button.textContent = PASTE.pair
  button.className = 'rounded-md bg-black px-3 py-1.5 font-medium text-white'
  form.append(input, button)
  form.addEventListener('submit', (event) => {
    event.preventDefault()
    const fragment = pastedPairing(input.value)
    if (!fragment) {
      input.value = ''
      input.placeholder = PASTE.wrong
      return
    }
    history.replaceState(null, '', `/${fragment}`)
    location.reload()
  })
  bar.appendChild(form)
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
  if (state.kind === 'unpaired' || state.kind === 'refused') pasteForm(bar)
  if (state.kind === 'connected') setTimeout(() => bar?.remove(), 2_500)
}

if (onRelay(document)) {
  installTunnel((state) => (document.body ? show(state) : addEventListener('DOMContentLoaded', () => show(state), { once: true })))
  if ('serviceWorker' in navigator) void navigator.serviceWorker.register('/sw.js').catch(() => undefined)
}
