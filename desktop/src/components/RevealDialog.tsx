import { Eye } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import { useT } from '../i18n'

/** Seconds the value stays on screen before the dialog closes itself. */
export const REVEAL_SECONDS = 30

type Shown = { status: 'loading' } | { status: 'value'; value: string } | { status: 'none' } | { status: 'error' }

/**
 * Shows one stored value after an explicit click. The value lives only in this component's
 * state (not in a store), and the dialog closes itself after REVEAL_SECONDS.
 */
export function RevealDialog({ name, onClose }: { name: string; onClose: () => void }) {
  const t = useT()
  const [shown, setShown] = useState<Shown>({ status: 'loading' })
  const [left, setLeft] = useState(REVEAL_SECONDS)

  useEffect(() => {
    let active = true
    api
      .revealValue(name)
      .then((value) => active && setShown(value === null ? { status: 'none' } : { status: 'value', value }))
      .catch(() => active && setShown({ status: 'error' }))
    return () => {
      active = false
      setShown({ status: 'loading' })
      api.hideValue()
    }
  }, [name])

  useEffect(() => {
    if (left <= 0) {
      onClose()
      return
    }
    const timer = setTimeout(() => setLeft((n) => n - 1), 1000)
    return () => clearTimeout(timer)
  }, [left, onClose])

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

        {shown.status === 'value' && <code className="revealed">{shown.value}</code>}
        {shown.status === 'none' && <p className="muted">{t('revealNone')}</p>}
        {shown.status === 'error' && <p className="hint hint-error">{t('revealError')}</p>}

        <p className="hint">{t('revealHint', { n: left })}</p>

        <div className="dialog-actions">
          <button type="button" className="btn btn-primary" onClick={onClose} autoFocus>
            {t('hide')}
          </button>
        </div>
      </div>
    </div>
  )
}
