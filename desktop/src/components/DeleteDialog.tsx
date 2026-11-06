import { Trash2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useT } from '../i18n'
import { useKeys } from '../stores/keys'

/** Confirms before deleting a key and its value. Cancel has the focus, so Enter does not delete. */
export function DeleteDialog({ name, onClose, onDeleted }: { name: string; onClose: () => void; onDeleted: () => void }) {
  const t = useT()
  const remove = useKeys((s) => s.remove)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const cancel = useRef<HTMLButtonElement>(null)

  useEffect(() => cancel.current?.focus(), [])
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  async function confirm() {
    if (busy) return
    setBusy(true)
    const failure = await remove(name)
    setBusy(false)
    if (failure) {
      setError(failure)
      return
    }
    onDeleted()
    onClose()
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-title" aria-describedby="delete-body">
        <div className="dialog-head">
          <span className="icon-badge icon-badge-danger">
            <Trash2 size={18} aria-hidden />
          </span>
          <h2 id="delete-title">{t('deleteTitle', { name })}</h2>
        </div>
        <p id="delete-body" className={error ? 'hint hint-error' : 'muted'}>
          {error ?? t('deleteBody')}
        </p>
        <div className="dialog-actions">
          <button ref={cancel} type="button" className="btn" onClick={onClose}>
            {t('cancel')}
          </button>
          <button type="button" className="btn btn-danger" disabled={busy} onClick={confirm}>
            {t('delete')}
          </button>
        </div>
      </div>
    </div>
  )
}
