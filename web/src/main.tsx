import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { loadReport } from './lib/report'
import './index.css'

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
