import { pagesFrom, type Brand } from './pages'

const files = import.meta.glob('../../docs/*/*.md', { query: '?raw', import: 'default', eager: true }) as Record<string, string>

export const PAGES = pagesFrom(files)

const WAIT_MS = 250
const TRIES = 20

export async function loadBrand(fetcher: typeof fetch = fetch, wait = WAIT_MS): Promise<Brand> {
  for (let attempt = 0; attempt < TRIES; attempt += 1) {
    try {
      const response = await fetcher('/api/docs')
      if (response.ok) return ((await response.json()) as { brand: Brand }).brand
      if (response.status !== 503) break
    } catch {
      break
    }
    await new Promise((resolve) => setTimeout(resolve, wait))
  }
  try {
    const instance = (await (await fetcher('/api/instance')).json()) as { name?: string }
    return instance.name ? { NAME: instance.name } : {}
  } catch {
    return {}
  }
}
