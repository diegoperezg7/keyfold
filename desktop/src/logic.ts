import type { SlotInfo } from './api'

export type StatusFilter = 'all' | 'stored' | 'empty'
export type SortKey = 'name' | 'status'
export interface Sort {
  key: SortKey
  dir: 'asc' | 'desc'
}

/** Status filter, then case-insensitive search on name or project, then sort. */
export function visibleKeys(keys: SlotInfo[], search: string, filter: StatusFilter, sort: Sort): SlotInfo[] {
  const query = search.trim().toLowerCase()
  const factor = sort.dir === 'asc' ? 1 : -1
  return keys
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
