import { FolderPlus, X } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import type { SlotInfo } from '../api'
import { useT } from '../i18n'
import { knownProjects } from '../logic'
import { useKeys } from '../stores/keys'

export function AssignDialog({ slot, onClose }: { slot: SlotInfo; onClose: () => void }) {
  const t = useT()
  const keys = useKeys((s) => s.keys)
  const assign = useKeys((s) => s.assign)
  const unassign = useKeys((s) => s.unassign)
  const [project, setProject] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  // The dialog follows the store, so a removed project leaves the chips at once.
  const current = keys.find((k) => k.name === slot.name) ?? slot
  const suggestions = knownProjects(keys).filter((p) => !current.projects.includes(p))

  useEffect(() => input.current?.focus(), [])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  async function submit(e: FormEvent) {
    e.preventDefault()
    if (!project.trim() || busy) return
    setBusy(true)
    const failure = await assign(slot.name, project)
    setBusy(false)
    if (failure) setError(failure)
    else onClose()
  }

  async function remove(p: string) {
    if (busy) return
    setBusy(true)
    const failure = await unassign(slot.name, p)
    setBusy(false)
    setError(failure)
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <form className="dialog" onSubmit={submit} role="dialog" aria-modal="true" aria-labelledby="assign-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <FolderPlus size={18} aria-hidden />
          </span>
          <h2 id="assign-title">{t('assignTitle', { name: slot.name })}</h2>
        </div>

        {current.projects.length > 0 && (
          <div className="field-row">
            <span className="label">{t('assignedTo')}</span>
            <span className="chips">
              {current.projects.map((p) => (
                <span key={p} className="chip chip-removable">
                  {p}
                  <button
                    type="button"
                    className="chip-remove"
                    aria-label={t('removeFrom', { project: p })}
                    title={t('removeFrom', { project: p })}
                    disabled={busy}
                    onClick={() => remove(p)}
                  >
                    <X size={12} aria-hidden />
                  </button>
                </span>
              ))}
            </span>
          </div>
        )}

        {suggestions.length > 0 && (
          <div className="field">
            <span className="label">{t('existingProjects')}</span>
            <div className="suggestions">
              {suggestions.map((p) => (
                <button
                  type="button"
                  key={p}
                  className={`chip chip-button ${project === p ? 'chip-selected' : ''}`}
                  onClick={() => {
                    setProject(p)
                    setError(null)
                  }}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>
        )}

        <label className="field">
          <input
            ref={input}
            className="input"
            value={project}
            placeholder={t('projectPlaceholder')}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            onChange={(e) => {
              setProject(e.target.value)
              setError(null)
            }}
          />
          <span className={error ? 'hint hint-error' : 'hint'}>{error ?? t('projectHint')}</span>
        </label>

        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>
            {t('cancel')}
          </button>
          <button type="submit" className="btn btn-primary" disabled={!project.trim() || busy}>
            {t('assign')}
          </button>
        </div>
      </form>
    </div>
  )
}
