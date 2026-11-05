import { Eye, Fingerprint } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import { useT } from '../i18n'

/** Seconds the value stays on screen before the dialog closes itself. */
export const REVEAL_SECONDS = 30

type Shown =
  | { status: 'unlocking' }
  | { status: 'value'; value: string }
  | { status: 'none' }
  | { status: 'locked' }
  | { status: 'error' }

/**
 * Shows one stored value after an explicit click. Rust asks for Touch ID or the password on
 * the first reveal while the window is open. The value lives only in this component's state
 * (not in a store), and the dialog closes itself REVEAL_SECONDS after it appears.
 */
export function RevealDialog({ name, onClose }: { name: string; onClose: () => void }) {
  const t = useT()
  const [shown, setShown] = useState<Shown>({ status: 'unlocking' })
  const [left, setLeft] = useState(REVEAL_SECONDS)

  useEffect(() => {
    let active = true
    api
      .revealValue(name)
      .then((value) => active && setShown(value === null ? { status: 'none' } : { status: 'value', value }))
      .catch((error) => active && setShown({ status: String(error) === 'locked' ? 'locked' : 'error' }))
    return () => {
      active = false
      api.hideValue()
    }
  }, [name])

  // The countdown only runs while a value is on screen.
  const counting = shown.status === 'value'
  useEffect(() => {
    if (!counting) return
    if (left <= 0) {
      onClose()
      return
    }
    const timer = setTimeout(() => setLeft((n) => n - 1), 1000)
    return () => clearTimeout(timer)
  }, [counting, left, onClose])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="reveal-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <Eye size={18} aria-hidden />
          </span>
          <h2 id="reveal-title">{t('promptTitle', { name })}</h2>
        </div>

        {shown.status === 'unlocking' && (
          <p className="unlocking">
            <Fingerprint size={20} aria-hidden />
            {t('revealUnlocking')}
          </p>
        )}
        {shown.status === 'value' && (
          <>
            <code className="revealed">{shown.value}</code>
            <div className="countdown" aria-hidden>
              <span style={{ animationDuration: `${REVEAL_SECONDS}s` }} />
            </div>
            <p className="hint">{t('revealHint', { n: left })}</p>
          </>
        )}
        {shown.status === 'none' && <p className="muted">{t('revealNone')}</p>}
        {shown.status === 'locked' && <p className="hint">{t('revealLocked')}</p>}
        {shown.status === 'error' && <p className="hint hint-error">{t('revealError')}</p>}

        <div className="dialog-actions">
          <button type="button" className="btn btn-primary" onClick={onClose} autoFocus>
            {t('hide')}
          </button>
        </div>
      </div>
    </div>
  )
}
