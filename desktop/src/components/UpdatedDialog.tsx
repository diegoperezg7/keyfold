import { CheckCircle2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, type UpdateInfo } from '../api'
import { useT } from '../i18n'

/**
 * Shown once, in the middle of the window, when the app comes back from installing an
 * update: confirms it worked and lists the release notes when the release has them.
 */
export function UpdatedDialog() {
  const t = useT()
  const [installed, setInstalled] = useState<UpdateInfo | null>(null)

  useEffect(() => {
    api
      .takeInstalledUpdate()
      .then(setInstalled)
      .catch(() => {})
  }, [])

  useEffect(() => {
    if (!installed) return
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && setInstalled(null)
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [installed])

  if (!installed) return null
  const notes = installed.notes.trim()
  const close = () => setInstalled(null)

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="updated-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <CheckCircle2 size={18} aria-hidden />
          </span>
          <h2 id="updated-title">{t('updatedTitle', { version: installed.version })}</h2>
        </div>
        <p className="muted">{t('updatedBody')}</p>
        {notes && (
          <div className="field">
            <span className="label">{t('updatedNotes')}</span>
            <div className="release-notes">{notes}</div>
          </div>
        )}
        <div className="dialog-actions">
          <button type="button" className="btn btn-primary" onClick={close} autoFocus>
            {t('updatedOk')}
          </button>
        </div>
      </div>
    </div>
  )
}
