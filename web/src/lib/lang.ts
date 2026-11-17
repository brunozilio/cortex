export type Lang = 'en' | 'pt'

export const LANGS: Lang[] = ['en', 'pt']

const KEY = 'docs-lang'

export function isLang(value: unknown): value is Lang {
  return value === 'en' || value === 'pt'
}

export function preferredLang(): Lang {
  try {
    const kept = localStorage.getItem(KEY)
    if (isLang(kept)) return kept
  } catch {
    return 'en'
  }
  return navigator.language.toLowerCase().startsWith('pt') ? 'pt' : 'en'
}

export function keepLang(lang: Lang): void {
  try {
    localStorage.setItem(KEY, lang)
  } catch {
    return
  }
}
