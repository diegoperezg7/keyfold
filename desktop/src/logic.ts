import type { SlotInfo } from './api'

export type StatusFilter = 'all' | 'stored' | 'empty'
export type SortKey = 'name' | 'status'
export interface Sort {
  key: SortKey
  dir: 'asc' | 'desc'
}

/** Project filter: null for every key, NO_PROJECT for keys assigned to none, or a project name. */
export type ProjectFilter = string | null
export const NO_PROJECT = '\u0000none'

export function inProject(key: SlotInfo, project: ProjectFilter): boolean {
  if (project === null) return true
  if (project === NO_PROJECT) return key.projects.length === 0
  return key.projects.includes(project)
}

/** Project filter, status filter, then case-insensitive search on name or project, then sort. */
export function visibleKeys(
  keys: SlotInfo[],
  search: string,
  filter: StatusFilter,
  sort: Sort,
  project: ProjectFilter = null,
): SlotInfo[] {
  const query = search.trim().toLowerCase()
  const factor = sort.dir === 'asc' ? 1 : -1
  return keys
    .filter((k) => inProject(k, project))
    .filter((k) => filter === 'all' || (filter === 'stored' ? k.hasValue : !k.hasValue))
    .filter(
      (k) =>
        !query || k.name.toLowerCase().includes(query) || k.projects.some((p) => p.toLowerCase().includes(query)),
    )
    .sort((a, b) => {
      // Status: empty first when ascending (they are the pending action). Name breaks ties.
      const byStatus = sort.key === 'status' ? Number(a.hasValue) - Number(b.hasValue) : 0
      return (byStatus || a.name.localeCompare(b.name)) * factor
    })
}

export function counts(keys: SlotInfo[]) {
  const stored = keys.filter((k) => k.hasValue).length
  return { all: keys.length, stored, empty: keys.length - stored }
}

/** Every project some key is assigned to, for quick picks when assigning. */
export function knownProjects(keys: SlotInfo[]): string[] {
  return [...new Set(keys.flatMap((k) => k.projects))].sort()
}

/** What the user types as a key name, in the form keys use: `exa api-key` -> `EXA_API_KEY`. */
export function normalizeKeyName(input: string): string {
  return input.trim().toUpperCase().replace(/[\s.-]+/g, '_')
}

/** Same rule as the Rust core: uppercase letters, digits and `_`, not starting with a digit. */
export function isValidKeyName(name: string): boolean {
  return /^[A-Z_][A-Z0-9_]*$/.test(name)
}

/** Same rule as the Rust core. */
export function isValidProject(project: string): boolean {
  return /^[A-Za-z0-9._-]+$/.test(project)
}
