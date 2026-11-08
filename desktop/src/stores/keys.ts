import { create } from 'zustand'
import { api, type SlotInfo } from '../api'
import { errorText } from '../i18n'
import type { ProjectFilter, Sort, SortKey, StatusFilter } from '../logic'

interface KeysState {
  keys: SlotInfo[]
  /** Folder path -> project (`keyfold link`). */
  folders: Record<string, string>
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
  /** Takes the key out of the project. Returns null on success, or the error message from Rust. */
  unassign: (name: string, project: string) => Promise<string | null>
  /** Deletes the key and its value. Returns null on success, or the error message from Rust. */
  remove: (name: string) => Promise<string | null>
  /** Removes a folder link. Returns null on success, or the error message from Rust. */
  unlink: (path: string) => Promise<string | null>
  setSearch: (search: string) => void
  setFilter: (filter: StatusFilter) => void
  toggleSort: (key: SortKey) => void
  project: ProjectFilter
  setProject: (project: ProjectFilter) => void
}

export const useKeys = create<KeysState>((set, get) => ({
  keys: [],
  folders: {},
  loaded: false,
  loadFailed: false,
  search: '',
  filter: 'all',
  sort: { key: 'name', dir: 'asc' },
  project: null,
  setProject: (project) => set({ project }),

  load: async () => {
    try {
      const [keys, folders] = await Promise.all([api.listKeys(), api.listFolders()])
      set({ keys, folders, loaded: true, loadFailed: false })
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
      return errorText(error)
    }
  },

  assign: async (name, project) => {
    try {
      await api.assignKey(name, project.trim())
      await get().load()
      return null
    } catch (error) {
      return errorText(error)
    }
  },

  unassign: async (name, project) => {
    try {
      await api.unassignKey(name, project)
      await get().load()
      return null
    } catch (error) {
      return errorText(error)
    }
  },

  remove: async (name) => {
    try {
      await api.deleteKey(name)
      await get().load()
      return null
    } catch (error) {
      return errorText(error)
    }
  },

  unlink: async (path) => {
    try {
      await api.unlinkFolder(path)
      await get().load()
      return null
    } catch (error) {
      return errorText(error)
    }
  },

  setSearch: (search) => set({ search }),
  setFilter: (filter) => set({ filter }),
  toggleSort: (key) =>
    set(({ sort }) => ({
      sort: sort.key === key ? { key, dir: sort.dir === 'asc' ? 'desc' : 'asc' } : { key, dir: 'asc' },
    })),
}))
