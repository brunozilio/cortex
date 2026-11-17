const CACHE = 'page-v1'
const PASSING = ['/api/', '/phone', '/machine', '/relay.json']

self.addEventListener('install', () => self.skipWaiting())

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((names) => Promise.all(names.filter((name) => name !== CACHE).map((name) => caches.delete(name))))
      .then(() => self.clients.claim()),
  )
})

async function fromNetwork(request, key) {
  const response = await fetch(request)
  if (response.ok) {
    const cache = await caches.open(CACHE)
    await cache.put(key ?? request, response.clone())
  }
  return response
}

async function networkFirst(request, key) {
  try {
    return await fromNetwork(request, key)
  } catch (error) {
    const kept = await caches.match(key ?? request)
    if (kept) return kept
    throw error
  }
}

async function cacheFirst(request) {
  return (await caches.match(request)) ?? fromNetwork(request)
}

self.addEventListener('fetch', (event) => {
  const request = event.request
  const url = new URL(request.url)
  if (request.method !== 'GET' || url.origin !== self.location.origin) return
  if (PASSING.some((path) => url.pathname.startsWith(path))) return
  if (request.mode === 'navigate') return event.respondWith(networkFirst(request, '/'))
  if (url.pathname.startsWith('/assets/')) return event.respondWith(cacheFirst(request))
  event.respondWith(networkFirst(request))
})
