import { beforeEach, describe, expect, it, vi } from 'vitest'

const saved: Array<{ name: string; value: string }> = []
let failNext = false

vi.mock('../api', () => ({
  // i18n (error messages) reads these at import.
  boot: { lang: 'en' },
  isWindows: false,
  api: {
    saveSecret: vi.fn(async (name: string, value: string) => {
      if (failNext) throw new Error('Keychain error -25293')
      saved.push({ name, value })
    }),
  },
}))

const { usePrompt } = await import('./prompt')
const secret = `exa-receive-${crypto.randomUUID()}`

/** Everything the paste window renders from, serialized. */
const viewState = () => JSON.stringify(usePrompt.getState())

describe('paste window state', () => {
  beforeEach(() => {
    saved.length = 0
    failNext = false
    usePrompt.getState().cancel()
  })

  it('opens empty, saves the pasted value once (Enter) and keeps no trace of it', async () => {
    expect(usePrompt.getState().value).toBe('')
    usePrompt.getState().markPasted()
    usePrompt.getState().setValue(`  ${secret}\n`)

    expect(await usePrompt.getState().submit('EXA_API_KEY', 'keyboard')).toBe(true)

    expect(saved).toEqual([{ name: 'EXA_API_KEY', value: secret }])
    expect(usePrompt.getState().value).toBe('')
    expect(viewState()).not.toContain(secret)
  })

  it('typed text + Enter does not save (stray keystrokes from another app); a click does', async () => {
    usePrompt.getState().setValue('git commit -m "fix"')
    expect(await usePrompt.getState().submit('EXA_API_KEY', 'keyboard')).toBe(false)
    expect(saved).toEqual([])

    usePrompt.getState().setValue(secret)
    expect(await usePrompt.getState().submit('EXA_API_KEY', 'click')).toBe(true)
    expect(saved).toEqual([{ name: 'EXA_API_KEY', value: secret }])
    expect(viewState()).not.toContain(secret)
  })

  it('cancel drops what was typed and stores nothing', () => {
    usePrompt.getState().setValue(secret)
    usePrompt.getState().cancel()
    expect(saved).toEqual([])
    expect(viewState()).not.toContain(secret)
  })

  it('a failed save reports an error without echoing the value', async () => {
    failNext = true
    usePrompt.getState().setValue(secret)
    expect(await usePrompt.getState().submit('EXA_API_KEY', 'click')).toBe(false)
    const { failed, error, value } = usePrompt.getState()
    expect(failed).toBe(true)
    expect(error).toBe('Keychain error -25293')
    // The typed value stays in the (masked) field so the user can retry; nothing else holds it.
    expect(value).toBe(secret)
    expect(JSON.stringify({ ...usePrompt.getState(), value: '' })).not.toContain(secret)
  })

  it('ignores empty input', async () => {
    usePrompt.getState().setValue('   ')
    expect(await usePrompt.getState().submit('EXA_API_KEY', 'click')).toBe(false)
    expect(saved).toEqual([])
  })
})
