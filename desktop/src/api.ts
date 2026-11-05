import { invoke } from '@tauri-apps/api/core'

/** What the UI may know about a key. There is no value field, by design. */
export interface SlotInfo {
  name: string
  reference: string
  hasValue: boolean
  projects: string[]
}

export interface Boot {
  view: 'keys' | 'prompt'
  name?: string | null
  project?: string | null
  lang?: 'es' | 'en'
}

declare global {
  interface Window {
    __KEYFOLD__?: Boot
  }
}

export const boot: Boot = (typeof window !== 'undefined' && window.__KEYFOLD__) || { view: 'keys' }

export const api = {
  listKeys: () => invoke<SlotInfo[]>('list_keys'),
  createKey: (name: string, project: string | null) => invoke<SlotInfo>('create_key', { name, project }),
  assignKey: (name: string, project: string) => invoke<SlotInfo>('assign_key', { name, project }),
  /** One-way: the value goes to the credential store and nothing comes back. */
  saveSecret: (name: string, value: string) => invoke<void>('save_secret', { name, value }),
  /** Keys window only, on a click. Null when the key has no value. Call hideValue when done. */
  revealValue: (name: string) => invoke<string | null>('reveal_value', { name }),
  hideValue: () => invoke<void>('hide_value'),
  finishPrompt: () => invoke<void>('finish_prompt'),
  openPrompt: (name: string) => invoke<void>('open_prompt', { name }),
}

export const isWindows = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
