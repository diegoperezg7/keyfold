import { create } from 'zustand'
import { api } from '../api'

/**
 * Paste window state. The value only lives in `value` while the user types;
 * it is cleared on save and on cancel, and never copied into `error`.
 *
 * The window takes focus when it opens, so keystrokes meant for another app
 * (a terminal) can land in the field. Enter only saves a value that arrived by
 * pasting; typed text needs an explicit click on Save.
 */
interface PromptState {
  value: string
  pasted: boolean
  saving: boolean
  failed: boolean
  setValue: (value: string) => void
  markPasted: () => void
  submit: (name: string, via: 'keyboard' | 'click') => Promise<boolean>
  cancel: () => void
}

export const usePrompt = create<PromptState>((set, get) => ({
  value: '',
  pasted: false,
  saving: false,
  failed: false,

  setValue: (value) => set({ value, failed: false, ...(value === '' ? { pasted: false } : {}) }),
  markPasted: () => set({ pasted: true }),

  submit: async (name, via) => {
    const value = get().value.trim()
    if (!value || get().saving) return false
    if (via === 'keyboard' && !get().pasted) return false
    set({ saving: true, failed: false })
    try {
      await api.saveSecret(name, value)
      set({ value: '', pasted: false, saving: false })
      return true
    } catch {
      set({ saving: false, failed: true })
      return false
    }
  },

  cancel: () => set({ value: '', pasted: false, saving: false, failed: false }),
}))
