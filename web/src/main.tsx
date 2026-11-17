import { StrictMode, Suspense, lazy } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { prepareAccess } from './lib/access'
import { onRelay } from './lib/tunnel/install'
import { STRINGS } from './lib/i18n'
import { preferredLang } from './lib/lang'
import { loadReport } from './lib/report'
import { LoadFailed } from './shell/LoadFailed'
import './index.css'

const DocsView = lazy(() => import('./views/docs/DocsView').then((module) => ({ default: module.DocsView })))
const onDocs = window.location.pathname === '/docs' || window.location.pathname.startsWith('/docs/')

function start() {
  if (!onDocs) {
    void fetch('/api/instance')
      .then((response) => (response.ok ? response.json() : null))
      .then((instance: { name?: string } | null) => {
        if (instance?.name) document.title = instance.name
      })
      .catch(() => undefined)
  }
  render()
}

function render() {
  const root = document.getElementById('root')
  if (root && onDocs) {
    createRoot(root).render(
      <StrictMode>
        <Suspense fallback={null}>
          <DocsView />
        </Suspense>
      </StrictMode>,
    )
  } else if (root) {
    const lang = preferredLang()
    const strings = STRINGS[lang]
    const sample = import.meta.env.DEV
      ? () => Promise.all([import('./sample'), import('./sample/insights')]).then(([data, insights]) => ({ report: data.sampleReport(), insights: insights.sampleInsights }))
      : undefined
    void loadReport(fetch, undefined, sample, strings).then((loaded) => {
      if (loaded.kind === 'live') document.body.dataset.live = 'on'
      createRoot(root).render(
        <StrictMode>
          {loaded.kind === 'failed' ? (
            <LoadFailed title={strings.loadFailedTitle} message={loaded.message} />
          ) : (
            <App initial={loaded.report} initialLang={lang} sample={loaded.kind === 'sample' ? loaded.insights : undefined} />
          )}
        </StrictMode>,
      )
    })
  }
}

void (onRelay(document) ? Promise.resolve() : prepareAccess()).finally(start)
