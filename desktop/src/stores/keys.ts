import { create } from 'zustand'
import { api, type SlotInfo } from '../api'
import type { Sort, SortKey, StatusFilter } from '../logic'

interface KeysState {
  keys: SlotInfo[]
  loaded: boolean
  loadFailed: boolean
  search: string
  filter: StatusFilter
  sort: Sort
  load: () => Promise<void>
  /** Creates an empty key. Returns null on success, or the error message from Rust. */
  create: (name: string, project: string) => Promise<string | null>
  /** Returns null on success, or the error message from Rust. */
  assign: (name: string, project: string) => Promise<string | null>
  setSearch: (search: string) => void
  setFilter: (filter: StatusFilter) => void
  toggleSort: (key: SortKey) => void
}

export const useKeys = create<KeysState>((set, get) => ({
  keys: [],
  loaded: false,
  loadFailed: false,
  search: '',
  filter: 'all',
  sort: { key: 'name', dir: 'asc' },

  load: async () => {
    try {
      set({ keys: await api.listKeys(), loaded: true, loadFailed: false })
    } catch {
      set({ loaded: true, loadFailed: true })
    }
  },

  create: async (name, project) => {
    try {
      await api.createKey(name, project.trim() || null)
      await get().load()
      return null
    } catch (error) {
      return String(error)
    }
  },

  assign: async (name, project) => {
    try {
      await api.assignKey(name, project.trim())
      await get().load()
      return null
    } catch (error) {
      return String(error)
    }
  },

  setSearch: (search) => set({ search }),
  setFilter: (filter) => set({ filter }),
  toggleSort: (key) =>
    set(({ sort }) => ({
      sort: sort.key === key ? { key, dir: sort.dir === 'asc' ? 'desc' : 'asc' } : { key, dir: 'asc' },
    })),
}))
