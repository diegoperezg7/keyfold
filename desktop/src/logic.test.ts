import { describe, expect, it } from 'vitest'
import type { SlotInfo } from './api'
import {
  counts,
  isPublicKey,
  parseRecipients,
  downloadPercent,
  isValidKeyName,
  isValidProject,
  knownProjects,
  looksLikePlaceholder,
  NO_PROJECT,
  normalizeKeyName,
  visibleKeys,
} from './logic'

const slot = (name: string, hasValue: boolean, projects: string[] = []): SlotInfo => ({
  name,
  reference: `keyfold://${name}`,
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

describe('project filter', () => {
  const many = [...keys, slot('STRIPE_KEY', false, ['shop', 'web'])]

  it('shows only the keys of the chosen project', () => {
    expect(visibleKeys(many, '', 'all', byName, 'web').map((k) => k.name)).toEqual(['OPENAI_API_KEY', 'STRIPE_KEY'])
    expect(visibleKeys(many, '', 'all', byName, 'shop').map((k) => k.name)).toEqual(['STRIPE_KEY'])
    expect(visibleKeys(many, '', 'all', byName, 'search-bot').map((k) => k.name)).toEqual(['EXA_API_KEY'])
  })

  it('lists keys with no project, and combines with status and search', () => {
    expect(visibleKeys(many, '', 'all', byName, NO_PROJECT).map((k) => k.name)).toEqual(['RESEND_API_KEY'])
    expect(visibleKeys(many, '', 'empty', byName, 'web').map((k) => k.name)).toEqual(['STRIPE_KEY'])
    expect(visibleKeys(many, 'openai', 'all', byName, 'shop')).toEqual([])
    expect(visibleKeys(many, '', 'all', byName, null)).toHaveLength(4)
  })
})

describe('new key form', () => {
  it('normalizes typed names to the key form', () => {
    expect(normalizeKeyName(' exa api-key ')).toBe('EXA_API_KEY')
    expect(normalizeKeyName('stripe.secret key')).toBe('STRIPE_SECRET_KEY')
  })

  it('validates names and projects like the core', () => {
    expect(isValidKeyName('EXA_API_KEY')).toBe(true)
    expect(isValidKeyName('1KEY')).toBe(false)
    expect(isValidKeyName('')).toBe(false)
    expect(isValidKeyName('KEY/2')).toBe(false)
    expect(isValidProject('search-bot')).toBe(true)
    expect(isValidProject('bad/project')).toBe(false)
  })
})

describe('placeholder values', () => {
  it('flags examples and values that are not random', () => {
    for (const v of ['sk-xxxxxxxxxxxxxxxxxxxx', 'your_api_key_here', 'changeme', '<OPENAI_API_KEY>', '${EXA_API_KEY}', 'aaaaaaaaaaaaaaaa', '***']) {
      expect(looksLikePlaceholder(v)).toBe(true)
    }
  })

  it('does not flag keys that look random, nor an empty field', () => {
    for (const v of ['sk-proj-Q7vN2kLp9RtW4mZx8YcB3dFh6JsG1aEu', 'AKIAZ7QW3XN5PLR8MV2K', 'ghp_9fK2mQ7xVb4nR1tY6wL3pZ8cH5jD0sAeUgOi', '']) { // gitleaks:allow (made-up test values)
      expect(looksLikePlaceholder(v)).toBe(false)
    }
  })
})

describe('update download', () => {
  it('reports whole percents, capped at 100', () => {
    expect(downloadPercent(0, 1000)).toBe(0)
    expect(downloadPercent(456, 1000)).toBe(45)
    expect(downloadPercent(1000, 1000)).toBe(100)
    expect(downloadPercent(1200, 1000)).toBe(100)
  })

  it('has no percent when the server does not send the size', () => {
    expect(downloadPercent(500, null)).toBeNull()
    expect(downloadPercent(500, 0)).toBeNull()
  })
})

describe('share recipients', () => {
  const KEY = 'age1qyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqszqgpqyqs3290gq'

  it('reads one public key per line, ignoring blanks, commas and repeats', () => {
    expect(parseRecipients(`${KEY}\n\n  ${KEY}, age1other `)).toEqual([KEY, 'age1other'])
    expect(parseRecipients('   ')).toEqual([])
  })

  it('accepts only age public keys', () => {
    expect(isPublicKey(KEY)).toBe(true)
    expect(isPublicKey('age1short')).toBe(false)
    expect(isPublicKey('AGE-SECRET-KEY-1QQQ')).toBe(false)
    expect(isPublicKey(KEY.toUpperCase())).toBe(false)
  })
})
