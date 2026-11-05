import { KeyRound } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api } from '../api'
import { useT } from '../i18n'
import { isValidKeyName, isValidProject, knownProjects, normalizeKeyName } from '../logic'
import { useKeys } from '../stores/keys'

/** Creates a key by hand. The value is never typed here: on create, the paste window opens. */
export function NewKeyDialog({ onClose }: { onClose: () => void }) {
  const t = useT()
  const keys = useKeys((s) => s.keys)
  const create = useKeys((s) => s.create)
  const [rawName, setRawName] = useState('')
  const [project, setProject] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const input = useRef<HTMLInputElement>(null)

  const name = normalizeKeyName(rawName)
  const exists = keys.some((k) => k.name === name)
  const projectOk = !project.trim() || isValidProject(project.trim())
  const nameError = rawName.trim() && !isValidKeyName(name) ? t('nameInvalid') : exists ? t('nameExists') : null
  const canCreate = isValidKeyName(name) && !exists && projectOk && !busy
  const projects = knownProjects(keys)

  useEffect(() => input.current?.focus(), [])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  async function submit(e: FormEvent) {
    e.preventDefault()
    if (!canCreate) return
    setBusy(true)
    const failure = await create(name, project)
    setBusy(false)
    if (failure) {
      setError(failure)
      return
    }
    onClose()
    api.openPrompt(name)
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <form className="dialog" onSubmit={submit} role="dialog" aria-modal="true" aria-labelledby="new-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <KeyRound size={18} aria-hidden />
          </span>
          <h2 id="new-title">{t('newKeyTitle')}</h2>
        </div>

        <label className="field">
          <span className="label">{t('colName')}</span>
          <input
            ref={input}
            className="input mono"
            value={rawName}
            placeholder="EXA_API_KEY"
            spellCheck={false}
            autoCapitalize="characters"
            autoComplete="off"
            onChange={(e) => {
              setRawName(e.target.value)
              setError(null)
            }}
          />
          <span className={nameError ? 'hint hint-error' : 'hint'}>
            {nameError ?? (name && name !== rawName.trim() ? t('nameAs', { name }) : t('nameHint'))}
          </span>
        </label>

        <label className="field">
          <span className="label">{t('projectOptional')}</span>
          <input
            className="input"
            value={project}
            list="known-projects"
            placeholder={t('projectPlaceholder')}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            onChange={(e) => {
              setProject(e.target.value)
              setError(null)
            }}
          />
          <datalist id="known-projects">
            {projects.map((p) => (
              <option key={p} value={p} />
            ))}
          </datalist>
          <span className={!projectOk || error ? 'hint hint-error' : 'hint'}>
            {error ?? (projectOk ? t('newKeyHint') : t('projectHint'))}
          </span>
        </label>

        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>
            {t('cancel')}
          </button>
          <button type="submit" className="btn btn-primary" disabled={!canCreate}>
            {t('createAndPaste')}
          </button>
        </div>
      </form>
    </div>
  )
}
