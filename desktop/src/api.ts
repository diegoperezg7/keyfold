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
  /** "new": open the New key dialog (from the menu bar). */
  action?: 'new' | null
  /** One-line result to show once (menu bar "Connect agent"). */
  notice?: Notice | null
}

/** A newer release than the running one. */
export interface UpdateInfo {
  version: string
  /** Release notes; empty when the release has none. */
  notes: string
}

/** Payload of the `update-progress` event while an update downloads. */
export interface UpdateProgress {
  downloaded: number
  total: number | null
}

export interface Notice {
  ok: boolean
  text: string
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
  /** Folder path -> project, linked on this machine. */
  listFolders: () => invoke<Record<string, string>>('list_folders'),
  unlinkFolder: (path: string) => invoke<boolean>('unlink_folder', { path }),
  unassignKey: (name: string, project: string) => invoke<SlotInfo>('unassign_key', { name, project }),
  /** One-way: the value goes to the credential store and nothing comes back. */
  saveSecret: (name: string, value: string) => invoke<void>('save_secret', { name, value }),
  /** Keys window only, on a click. Null when the key has no value. Call hideValue when done. */
  revealValue: (name: string) => invoke<string | null>('reveal_value', { name }),
  hideValue: () => invoke<void>('hide_value'),
  finishPrompt: () => invoke<void>('finish_prompt'),
  /** This computer's public key (age1…). Safe to show and send. */
  publicKey: () => invoke<string>('public_key'),
  /** Encrypted package for `recipients`; empty `names` means every key with a value. Asks for Touch ID. */
  shareKeys: (names: string[], recipients: string[]) => invoke<string>('share_keys', { names, recipients }),
  /** Opens a package made for this computer; returns names and projects only. */
  importPackage: (pkg: string) => invoke<SlotInfo[]>('import_package', { package: pkg }),
  openPrompt: (name: string) => invoke<void>('open_prompt', { name }),
  /** Null when this is the latest version. */
  checkUpdate: () => invoke<UpdateInfo | null>('check_update'),
  /** Downloads, installs and restarts the app; it only returns if something fails. */
  installUpdate: () => invoke<void>('install_update'),
  /** Right after the restart that installed an update: that update, once. Null otherwise. */
  takeInstalledUpdate: () => invoke<UpdateInfo | null>('take_installed_update'),
}

export const isWindows = typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
