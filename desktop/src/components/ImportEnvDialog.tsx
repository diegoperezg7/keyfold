import { FolderSearch } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, type EnvScan, type SlotInfo } from '../api'
import { errorText, useT } from '../i18n'
import { SuccessMark } from './SuccessMark'
import { envGroups, envPicks, initialEnvPicks, toggleEnvPick } from '../logic'

type Status = { kind: 'idle' | 'busy' } | { kind: 'done' | 'error'; text: string }

/**
 * First load from the `.env` files of your projects. Rust opens the folder picker, reads the
 * files and returns names only; the ticked rows go back as (file, name) and Rust reads the
 * values again. No value reaches this component.
 */
export function ImportEnvDialog({ onClose }: { onClose: () => void }) {
  const t = useT()
  const [scan, setScan] = useState<EnvScan | null>(null)
  const [picks, setPicks] = useState<Set<number>>(new Set())
  const [status, setStatus] = useState<Status>({ kind: 'idle' })
  /** The keys stored by the last import: the dialog shows them instead of the picker. */
  const [done, setDone] = useState<SlotInfo[] | null>(null)

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  async function pick() {
    setStatus({ kind: 'busy' })
    try {
      const result = await api.scanEnv()
      if (result) {
        setScan(result)
        setPicks(initialEnvPicks(result.candidates))
      }
      setStatus({ kind: 'idle' })
    } catch (error) {
      setStatus({ kind: 'error', text: errorText(error) })
    }
  }

  async function importIt() {
    if (!scan) return
    setStatus({ kind: 'busy' })
    try {
      const imported = await api.importEnv(envPicks(scan.candidates, picks))
      setScan(null)
      setPicks(new Set())
      setDone(imported)
      setStatus({ kind: 'idle' })
    } catch (error) {
      setStatus({ kind: 'error', text: errorText(error) })
    }
  }

  const busy = status.kind === 'busy'

  if (done) {
    return (
      <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
        <div className="dialog env-done" role="dialog" aria-modal="true" aria-labelledby="env-done-title">
          <SuccessMark />
          <h2 id="env-done-title">{t(done.length === 1 ? 'envDoneOne' : 'envDoneOther', { n: done.length })}</h2>
          <p className="muted small mono">{done.map((k) => k.name).join(' · ')}</p>
          <div className="dialog-actions">
            <button type="button" className="btn btn-primary" autoFocus onClick={onClose}>
              {t('envDoneOk')}
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog dialog-wide" role="dialog" aria-modal="true" aria-labelledby="env-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <FolderSearch size={18} aria-hidden />
          </span>
          <h2 id="env-title">{t('envImport')}</h2>
        </div>

        {!scan ? (
          <p className="muted">{t('envIntro')}</p>
        ) : scan.candidates.length === 0 ? (
          <p className="muted">
            {t('envNone')}
            {scan.truncated && ` ${t('envTruncated')}`}
          </p>
        ) : (
          <>
            <p className="muted small">
              {t(scan.files.length === 1 ? 'envFilesOne' : 'envFilesOther', { n: scan.files.length })},{' '}
              {t(scan.candidates.length === 1 ? 'envVarsOne' : 'envVarsOther', { n: scan.candidates.length })}.
              {scan.truncated && ` ${t('envTruncated')}`}
            </p>
            <div className="env-list">
              {envGroups(scan.candidates).map(({ project, rows }) => (
                <div key={project ?? ''} className="env-group">
                  <span className="label">{project ?? t('noProject')}</span>
                  {rows.map((i) => {
                    const c = scan.candidates[i]
                    const note = !c.validName
                      ? t('envInvalidName')
                      : [c.exists && t('envExists'), c.conflict && t('envConflict'), c.placeholder && t('envPlaceholder')]
                          .filter(Boolean)
                          .join(' · ')
                    return (
                      <label key={i} className={c.validName ? 'env-row' : 'env-row env-row-off'} title={c.file}>
                        <input
                          type="checkbox"
                          checked={picks.has(i)}
                          disabled={!c.validName || busy}
                          onChange={() => setPicks(toggleEnvPick(scan.candidates, picks, i))}
                        />
                        <code>{c.name}</code>
                        <span className="muted small">{c.file.split(/[\\/]/).pop()}</span>
                        {note && <span className="hint-warn small">{note}</span>}
                      </label>
                    )
                  })}
                </div>
              ))}
            </div>
          </>
        )}

        {(status.kind === 'done' || status.kind === 'error') && (
          <p className={status.kind === 'error' ? 'hint hint-error' : 'hint'}>{status.text}</p>
        )}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>
            {t('close')}
          </button>
          <button type="button" className={scan?.candidates.length ? 'btn' : 'btn btn-primary'} disabled={busy} onClick={pick}>
            {busy && !scan ? t('envScanning') : t('envPick')}
          </button>
          {!!scan?.candidates.length && (
            <button type="button" className="btn btn-primary" disabled={busy || picks.size === 0} onClick={importIt}>
              {t('envImportN', { n: picks.size })}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
