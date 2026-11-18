import type { Lang } from './pages'

export interface Category {
  label: Record<Lang, string>
  slugs: string[]
}

export const SIDEBAR: Category[] = [
  { label: { en: 'Getting started', pt: 'Primeiros passos' }, slugs: ['introduction', 'quickstart'] },
  {
    label: { en: 'Using the chat', pt: 'Usando o chat' },
    slugs: ['chat', 'keyboard-shortcuts', 'slash-commands', 'sessions', 'output-styles'],
  },
  {
    label: { en: 'Models and accounts', pt: 'Modelos e contas' },
    slugs: ['models-and-providers', 'accounts'],
  },
  {
    label: { en: 'Configuration', pt: 'Configuração' },
    slugs: ['settings', 'permissions', 'sandbox-and-safety', 'environment-variables', 'files-and-directories', 'instructions-files'],
  },
  {
    label: { en: 'Extending', pt: 'Extensões' },
    slugs: ['skills', 'agents', 'hooks', 'mcp', 'channels', 'plugins', 'statusline'],
  },
  {
    label: { en: 'Features', pt: 'Recursos' },
    slugs: [
      'experimental-features',
      'memory',
      'budget-and-cost',
      'branches-and-git',
      'verification-and-review',
      'learning',
      'schedules',
      'code-intelligence',
      'trace-console',
      'remote-control',
    ],
  },
  { label: { en: 'Automation', pt: 'Automação' }, slugs: ['headless', 'evals'] },
  { label: { en: 'Reference', pt: 'Referência' }, slugs: ['cli-reference', 'troubleshooting'] },
]

export const ORDER: string[] = SIDEBAR.flatMap((category) => category.slugs)

export function neighbours(slug: string): { previous?: string; next?: string } {
  const at = ORDER.indexOf(slug)
  if (at < 0) return {}
  return { previous: ORDER[at - 1], next: ORDER[at + 1] }
}

export function categoryOf(slug: string): Category | undefined {
  return SIDEBAR.find((category) => category.slugs.includes(slug))
}
