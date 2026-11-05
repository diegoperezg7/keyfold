import '@fontsource-variable/inter'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App'
import { useI18n } from './i18n'
import './styles.css'

document.documentElement.lang = useI18n.getState().lang
// macOS windows draw their content under the title bar (see unified_title_bar in gui.rs).
if (/Mac/i.test(navigator.userAgent)) document.documentElement.dataset.platform = 'mac'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
