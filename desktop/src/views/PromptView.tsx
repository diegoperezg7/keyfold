import { Lock, ShieldAlert, TriangleAlert } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api } from '../api'
import { Logo } from '../components/Logo'
import { useT } from '../i18n'
import { looksLikePlaceholder } from '../logic'
import { usePrompt } from '../stores/prompt'

/** How long the "saved" confirmation stays before the window closes (it fades out at the end). */
const SUCCESS_MS = 1300

/** Circle and check that draw themselves (see .success-mark in styles.css). */
function SuccessMark() {
  return (
    <svg className="success-mark" width="64" height="64" viewBox="0 0 64 64" aria-hidden>
      <circle className="success-ring" cx="32" cy="32" r="29" />
      <path className="success-check" d="M20 33 l8 8 l16 -17" />
    </svg>
  )
}

/** The paste window. A password field only: no reveal toggle, no copy. */
export function PromptView({ name, project }: { name: string; project?: string | null }) {
  const t = useT()
  const { value, pasted, saving, failed, setValue, markPasted, submit, cancel } = usePrompt()
  const [done, setDone] = useState(false)
  const input = useRef<HTMLInputElement>(null)
  const placeholder = looksLikePlaceholder(value)

  useEffect(() => {
    input.current?.focus()
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  function close() {
    cancel()
    api.finishPrompt()
  }

  async function save(via: 'keyboard' | 'click') {
    // Only a successful save shows the confirmation; a failure keeps the form and its error.
    if (await submit(name, via)) {
      setDone(true)
      setTimeout(() => api.finishPrompt(), SUCCESS_MS)
    }
  }

  // Enter is handled on the field (keyboard path); the form itself never submits.
  const onSubmit = (e: FormEvent) => e.preventDefault()

  if (done) {
    return (
      <div className="prompt prompt-success" role="status" aria-live="polite">
        <SuccessMark />
        <h1>{t('savedTitle')}</h1>
        <p className="muted small">{t('savedBody', { name })}</p>
      </div>
    )
  }

  return (
    <form className="prompt" onSubmit={onSubmit}>
      <div className="drag-strip" data-tauri-drag-region />
      <div className="prompt-head">
        <Logo size={44} />
        <div>
          <h1>{t('promptTitle', { name })}</h1>
          {project && <p className="muted small">{t('promptProject', { project })}</p>}
        </div>
      </div>
      <p className="warning">
        <ShieldAlert size={14} aria-hidden /> {t('promptWarning')}
      </p>

      <input
        ref={input}
        className="input input-secret"
        type="password"
        value={value}
        placeholder={t('promptPlaceholder')}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
        disabled={saving}
        onChange={(e) => setValue(e.target.value)}
        onPaste={() => markPasted()}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            save('keyboard')
          }
        }}
        onCopy={(e) => e.preventDefault()}
        onCut={(e) => e.preventDefault()}
        onDragStart={(e) => e.preventDefault()}
      />

      <p className={failed ? 'hint hint-error' : placeholder ? 'hint hint-warn' : 'hint'}>
        {failed ? (
          t('saveError')
        ) : placeholder ? (
          <>
            <TriangleAlert size={12} aria-hidden /> {t('placeholderHint')}
          </>
        ) : value && !pasted ? (
          t('typedHint')
        ) : (
          <>
            <Lock size={12} aria-hidden /> {t('promptHint', { name })}
          </>
        )}
      </p>

      <div className="dialog-actions">
        <button type="button" className="btn" onClick={close} disabled={saving}>
          {t('cancel')}
        </button>
        <button type="button" className="btn btn-primary" onClick={() => save('click')} disabled={!value.trim() || saving}>
          {saving ? t('saving') : t('save')}
        </button>
      </div>
    </form>
  )
}
