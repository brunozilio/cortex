import type { Lang } from './pages'

const STRINGS = {
  docs: { en: 'Docs', pt: 'Documentação' },
  search: { en: 'Search the docs', pt: 'Buscar na documentação' },
  searchHint: { en: 'Type to search pages, sections, commands, settings…', pt: 'Digite para buscar páginas, seções, comandos, configurações…' },
  noResults: { en: 'No results for', pt: 'Nenhum resultado para' },
  onThisPage: { en: 'On this page', pt: 'Nesta página' },
  previous: { en: 'Previous', pt: 'Anterior' },
  next: { en: 'Next', pt: 'Próxima' },
  notFound: { en: 'This page does not exist.', pt: 'Esta página não existe.' },
  notTranslated: { en: 'This page is not translated yet; showing English.', pt: 'Esta página ainda não foi traduzida; mostrando em inglês.' },
  copy: { en: 'Copy', pt: 'Copiar' },
  copied: { en: 'Copied', pt: 'Copiado' },
  theme: { en: 'Toggle theme', pt: 'Alternar tema' },
  menu: { en: 'Menu', pt: 'Menu' },
  navigate: { en: 'to navigate', pt: 'para navegar' },
  open: { en: 'to open', pt: 'para abrir' },
  close: { en: 'to close', pt: 'para fechar' },
  NOTE: { en: 'Note', pt: 'Nota' },
  TIP: { en: 'Tip', pt: 'Dica' },
  IMPORTANT: { en: 'Important', pt: 'Importante' },
  WARNING: { en: 'Warning', pt: 'Atenção' },
  CAUTION: { en: 'Caution', pt: 'Cuidado' },
  EXPERIMENTAL: { en: 'Experimental', pt: 'Experimental' },
} satisfies Record<string, Record<Lang, string>>

export type Word = keyof typeof STRINGS

export function say(lang: Lang, word: Word): string {
  return STRINGS[word][lang]
}
