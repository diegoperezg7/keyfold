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

/**
 * Values people put in docs and examples instead of a real key. Pattern and entropy threshold
 * come from mcp-guard (src/core/secrets.ts).
 */
const PLACEHOLDER =
  /x{4,}|X{4,}|\*{3,}|\.{3}|0{6,}|1234|abcd|example|sample|dummy|placeholder|redacted|changeme|your[_-]?|<[^>]*>|\$\{|\{\{/i

/** A pasted value that is probably not a real key: a placeholder or text that is not random. */
export function looksLikePlaceholder(value: string): boolean {
  const v = value.trim()
  return v !== '' && (PLACEHOLDER.test(v) || shannonEntropy(v) < 3)
}

/** Bits per character. Random base64 sits near 6, English words near 3. */
export function shannonEntropy(s: string): number {
  if (!s) return 0
  const counts = new Map<string, number>()
  for (const ch of s) counts.set(ch, (counts.get(ch) ?? 0) + 1)
  let h = 0
  for (const n of counts.values()) {
    const p = n / s.length
    h -= p * Math.log2(p)
  }
  return h
}
