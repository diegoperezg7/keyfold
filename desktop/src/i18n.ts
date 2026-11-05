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
  emptyBody: 'Pulsa Nueva clave o pide a tu agente que cree una.',
  noResultsTitle: 'Sin resultados',
  noResultsBody: 'Ninguna clave coincide con la búsqueda o el filtro.',
  footerNote: 'Cifradas en el {store}. Los valores quedan ocultos hasta que pulses Ver.',
  revealValue: 'Ver valor',
  show: 'Ver',
  revealUnlocking: 'Confirma con Touch ID o con tu contraseña.',
  revealLocked: 'Desbloqueo cancelado. El valor sigue oculto.',
  revealHint: 'Se oculta en {n} s. Mientras se muestra, esta ventana no sale en capturas de pantalla.',
  revealNone: 'Esta clave aún no tiene valor.',
  revealError: 'No se pudo leer el valor.',
  hide: 'Ocultar',
  newKey: 'Nueva clave',
  newKeyTitle: 'Nueva clave',
  nameHint: 'Mayúsculas, números y guion bajo, como en el entorno.',
  nameAs: 'Se guardará como {name}',
  nameInvalid: 'Usa letras, números y guion bajo; no empieces por un número.',
  nameExists: 'Ya existe una clave con ese nombre.',
  projectOptional: 'Proyecto (opcional)',
  newKeyHint: 'Al crearla se abre la ventana para pegar el valor.',
  createAndPaste: 'Crear y pegar valor',
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
  promptHint: 'Se guarda cifrado en el {store}. El agente solo recibe la referencia keyfold://{name}.',
  save: 'Guardar',
  saving: 'Guardando…',
  savedTitle: 'Clave guardada',
  savedBody: 'Tu agente ya puede usar keyfold://{name}.',
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
  emptyBody: 'Click New key, or ask your agent to create one.',
  noResultsTitle: 'No results',
  noResultsBody: 'No key matches the search or the filter.',
  footerNote: 'Encrypted in the {store}. Values stay hidden until you click Show.',
  revealValue: 'Show value',
  show: 'Show',
  revealUnlocking: 'Confirm with Touch ID or your password.',
  revealLocked: 'Unlock cancelled. The value stays hidden.',
  revealHint: 'Hides in {n} s. While shown, this window is left out of screenshots.',
  revealNone: 'This key has no value yet.',
  revealError: 'Could not read the value.',
  hide: 'Hide',
  newKey: 'New key',
  newKeyTitle: 'New key',
  nameHint: 'Uppercase letters, digits and underscore, as in the environment.',
  nameAs: 'Saved as {name}',
  nameInvalid: 'Use letters, digits and underscore; do not start with a digit.',
  nameExists: 'A key with that name already exists.',
  projectOptional: 'Project (optional)',
  newKeyHint: 'Once created, the window to paste the value opens.',
  createAndPaste: 'Create and paste value',
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
  promptHint: 'Stored encrypted in the {store}. The agent only gets the reference keyfold://{name}.',
  save: 'Save',
  saving: 'Saving…',
  savedTitle: 'Key saved',
  savedBody: 'Your agent can now use keyfold://{name}.',
  saveError: 'Could not save. Try again.',
  typedHint: 'Typed by hand: click Save to confirm. Enter only saves what you paste.',
  promptWarning: 'Only paste keys you asked for. If you did not expect this window, cancel.',
}

const dictionaries: Record<Lang, Record<Key, string>> = { es, en }

function initialLang(): Lang {
  // Rust detects the OS language and passes it in; the browser language is a fallback.
  if (boot.lang) return boot.lang
  return typeof navigator !== 'undefined' && navigator.language.startsWith('es') ? 'es' : 'en'
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
