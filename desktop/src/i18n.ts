import { create } from 'zustand'
import { boot, isWindows } from './api'

export type Lang = 'es' | 'en'

const es = {
  searchPlaceholder: 'Buscar por nombre o proyecto',
  filterAll: 'Todas',
  filterStored: 'Guardadas',
  filterEmpty: 'Vacías',
  refresh: 'Actualizar',
  colName: 'Nombre',
  colStatus: 'Estado',
  colProjects: 'Proyectos',
  statusStored: 'guardado',
  statusEmpty: 'vacío',
  noProject: 'Sin proyecto',
  addValue: 'Añadir valor',
  changeValue: 'Cambiar valor',
  assignProject: 'Asignar proyecto',
  emptyTitle: 'Aún no hay claves',
  emptyBody: 'Pide a tu agente que cree una, o ejecuta: llavero create NOMBRE',
  noResultsTitle: 'Sin resultados',
  noResultsBody: 'Ninguna clave coincide con la búsqueda o el filtro.',
  footerNote: 'Cifradas en el {store}. Los valores nunca se muestran aquí.',
  storeMac: 'Llavero de macOS',
  storeWindows: 'Administrador de credenciales de Windows',
  keysOne: '{n} clave',
  keysOther: '{n} claves',
  storedOne: '{n} guardada',
  storedOther: '{n} guardadas',
  emptyOne: '{n} vacía',
  emptyOther: '{n} vacías',
  loadError: 'No se pudieron cargar las claves.',
  assignTitle: 'Asignar {name} a un proyecto',
  assignedTo: 'Ya asignada a',
  existingProjects: 'Proyectos existentes',
  projectPlaceholder: 'nombre-del-proyecto',
  projectHint: 'Letras, números, punto, guion o guion bajo.',
  cancel: 'Cancelar',
  assign: 'Asignar',
  promptTitle: 'Valor de {name}',
  promptProject: 'Proyecto: {project}',
  promptPlaceholder: 'Pega aquí la clave',
  promptHint: 'Se guarda cifrado en el {store}. El agente solo recibe la referencia llavero://{name}.',
  save: 'Guardar',
  saving: 'Guardando…',
  saved: 'Guardado',
  saveError: 'No se pudo guardar. Inténtalo de nuevo.',
  typedHint: 'Escrito a mano: pulsa Guardar para confirmar. Intro solo guarda lo que pegas.',
  promptWarning: 'Pega solo claves que hayas pedido tú. Si no esperabas esta ventana, cancela.',
} as const

export type Key = keyof typeof es

const en: Record<Key, string> = {
  searchPlaceholder: 'Search by name or project',
  filterAll: 'All',
  filterStored: 'Stored',
  filterEmpty: 'Empty',
  refresh: 'Refresh',
  colName: 'Name',
  colStatus: 'Status',
  colProjects: 'Projects',
  statusStored: 'stored',
  statusEmpty: 'empty',
  noProject: 'No project',
  addValue: 'Add value',
  changeValue: 'Change value',
  assignProject: 'Assign project',
  emptyTitle: 'No keys yet',
  emptyBody: 'Ask your agent to create one, or run: llavero create NAME',
  noResultsTitle: 'No results',
  noResultsBody: 'No key matches the search or the filter.',
  footerNote: 'Encrypted in the {store}. Values are never shown here.',
  storeMac: 'macOS Keychain',
  storeWindows: 'Windows Credential Manager',
  keysOne: '{n} key',
  keysOther: '{n} keys',
  storedOne: '{n} stored',
  storedOther: '{n} stored',
  emptyOne: '{n} empty',
  emptyOther: '{n} empty',
  loadError: 'Could not load the keys.',
  assignTitle: 'Assign {name} to a project',
  assignedTo: 'Already assigned to',
  existingProjects: 'Existing projects',
  projectPlaceholder: 'project-name',
  projectHint: 'Letters, digits, dot, dash or underscore.',
  cancel: 'Cancel',
  assign: 'Assign',
  promptTitle: 'Value for {name}',
  promptProject: 'Project: {project}',
  promptPlaceholder: 'Paste the key here',
  promptHint: 'Stored encrypted in the {store}. The agent only gets the reference llavero://{name}.',
  save: 'Save',
  saving: 'Saving…',
  saved: 'Saved',
  saveError: 'Could not save. Try again.',
  typedHint: 'Typed by hand: click Save to confirm. Enter only saves what you paste.',
  promptWarning: 'Only paste keys you asked for. If you did not expect this window, cancel.',
}

const dictionaries: Record<Lang, Record<Key, string>> = { es, en }

function initialLang(): Lang {
  if (boot.lang) return boot.lang
  return typeof navigator !== 'undefined' && !navigator.language.startsWith('es') ? 'en' : 'es'
}

interface I18nState {
  lang: Lang
  setLang: (lang: Lang) => void
}

export const useI18n = create<I18nState>((set) => ({
  lang: initialLang(),
  setLang: (lang) => set({ lang }),
}))

export function translate(lang: Lang, key: Key, vars: Record<string, string | number> = {}): string {
  const store = dictionaries[lang][isWindows ? 'storeWindows' : 'storeMac']
  return dictionaries[lang][key].replace(/\{(\w+)\}/g, (_, name: string) =>
    name === 'store' ? store : String(vars[name] ?? ''),
  )
}

/** Hook: returns t(key, vars) bound to the current language. */
export function useT() {
  const lang = useI18n((s) => s.lang)
  return (key: Key, vars?: Record<string, string | number>) => translate(lang, key, vars)
}
