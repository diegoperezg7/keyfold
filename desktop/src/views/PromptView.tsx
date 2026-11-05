import { Check, KeyRound, Lock, ShieldAlert } from 'lucide-react'
import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api } from '../api'
import { useT } from '../i18n'
import { usePrompt } from '../stores/prompt'

/** The paste window. A password field only: no reveal toggle, no copy. */
export function PromptView({ name, project }: { name: string; project?: string | null }) {
  const t = useT()
  const { value, pasted, saving, failed, setValue, markPasted, submit, cancel } = usePrompt()
  const [done, setDone] = useState(false)
  const input = useRef<HTMLInputElement>(null)

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
    if (await submit(name, via)) {
      setDone(true)
      setTimeout(() => api.finishPrompt(), 450)
    }
  }

  // Enter is handled on the field (keyboard path); the form itself never submits.
  const onSubmit = (e: FormEvent) => e.preventDefault()

  return (
    <form className="prompt" onSubmit={onSubmit}>
      <div className="prompt-head">
        <span className="icon-badge icon-badge-lg">{done ? <Check size={22} aria-hidden /> : <KeyRound size={22} aria-hidden />}</span>
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
        disabled={saving || done}
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

      <p className={failed ? 'hint hint-error' : 'hint'}>
        {failed ? (
          t('saveError')
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
        <button type="button" className="btn btn-primary" onClick={() => save('click')} disabled={!value.trim() || saving || done}>
          {done ? t('saved') : saving ? t('saving') : t('save')}
        </button>
      </div>
    </form>
  )
}
