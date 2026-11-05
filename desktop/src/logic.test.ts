import { describe, expect, it } from 'vitest'
import type { SlotInfo } from './api'
import { counts, knownProjects, visibleKeys } from './logic'

const slot = (name: string, hasValue: boolean, projects: string[] = []): SlotInfo => ({
  name,
  reference: `llavero://${name}`,
  hasValue,
  projects,
})

const keys = [slot('OPENAI_API_KEY', true, ['web']), slot('RESEND_API_KEY', false), slot('EXA_API_KEY', true, ['search-bot'])]
const byName = { key: 'name', dir: 'asc' } as const

describe('keys window logic', () => {
  it('counts and filters by status', () => {
    expect(counts(keys)).toEqual({ all: 3, stored: 2, empty: 1 })
    expect(visibleKeys(keys, '', 'empty', byName).map((k) => k.name)).toEqual(['RESEND_API_KEY'])
    expect(visibleKeys(keys, '', 'stored', byName).map((k) => k.name)).toEqual(['EXA_API_KEY', 'OPENAI_API_KEY'])
  })

  it('searches name or project, case-insensitive', () => {
    expect(visibleKeys(keys, 'openai', 'all', byName).map((k) => k.name)).toEqual(['OPENAI_API_KEY'])
    expect(visibleKeys(keys, '  SEARCH-bot ', 'all', byName).map((k) => k.name)).toEqual(['EXA_API_KEY'])
    expect(visibleKeys(keys, 'nothing', 'all', byName)).toEqual([])
  })

  it('sorts by name and by status (empty first)', () => {
    expect(visibleKeys(keys, '', 'all', byName).map((k) => k.name)).toEqual(['EXA_API_KEY', 'OPENAI_API_KEY', 'RESEND_API_KEY'])
    expect(visibleKeys(keys, '', 'all', { key: 'name', dir: 'desc' }).map((k) => k.name)).toEqual([
      'RESEND_API_KEY',
      'OPENAI_API_KEY',
      'EXA_API_KEY',
    ])
    expect(visibleKeys(keys, '', 'all', { key: 'status', dir: 'asc' })[0].name).toBe('RESEND_API_KEY')
  })

  it('lists known projects once, sorted', () => {
    expect(knownProjects([...keys, slot('X_KEY', false, ['web'])])).toEqual(['search-bot', 'web'])
  })
})
