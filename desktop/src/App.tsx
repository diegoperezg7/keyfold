import { boot } from './api'
import { KeysView } from './views/KeysView'
import { PromptView } from './views/PromptView'

export function App() {
  if (boot.view === 'prompt' && boot.name) {
    return <PromptView name={boot.name} project={boot.project} />
  }
  return <KeysView />
}
