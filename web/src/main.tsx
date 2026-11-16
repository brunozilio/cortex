import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { loadReport } from './lib/report'
import './index.css'

void fetch('/api/instance')
  .then((response) => (response.ok ? response.json() : null))
  .then((instance: { name?: string } | null) => {
    if (instance?.name) document.title = instance.name
  })
  .catch(() => undefined)

const root = document.getElementById('root')
if (root) {
  void loadReport().then(({ report, live }) => {
    if (live) document.body.dataset.live = 'on'
    createRoot(root).render(
      <StrictMode>
        <App initial={report} />
      </StrictMode>,
    )
  })
}
