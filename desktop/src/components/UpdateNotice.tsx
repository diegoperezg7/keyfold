import { listen } from '@tauri-apps/api/event'
import { AlertCircle, Download } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api, type UpdateProgress } from '../api'
import { useT } from '../i18n'
import { downloadPercent } from '../logic'

type Phase =
  | { status: 'available' }
  | { status: 'downloading'; downloaded: number; total: number | null }
  | { status: 'error' }

/**
 * Bottom-right card when a newer release exists. "Update" downloads it with a progress bar,
 * installs it and restarts the app (all in Rust, see install_update in gui.rs).
 * Checks once each time the keys window opens; offline or no update shows nothing.
 */
export function UpdateNotice() {
  const t = useT()
  const [version, setVersion] = useState<string | null>(null)
  const [phase, setPhase] = useState<Phase>({ status: 'available' })

  useEffect(() => {
    let active = true
    api
      .checkUpdate()
      .then((info) => active && setVersion(info?.version ?? null))
      .catch(() => {})
    const stop = listen<UpdateProgress>('update-progress', (e) =>
      setPhase({ status: 'downloading', downloaded: e.payload.downloaded, total: e.payload.total }),
    )
    return () => {
      active = false
      stop.then((f) => f())
    }
  }, [])

  if (!version) return null

  const install = () => {
    setPhase({ status: 'downloading', downloaded: 0, total: null })
    // On success the app restarts, so only a failure comes back here.
    api.installUpdate().catch(() => setPhase({ status: 'error' }))
  }

  const downloading = phase.status === 'downloading'
  const percent = downloading ? downloadPercent(phase.downloaded, phase.total) : null
  const done = percent === 100

  return (
    <aside className="update" role="status" aria-live="polite">
      <div className="update-head">
        <span className="icon-badge">
          {phase.status === 'error' ? <AlertCircle size={18} aria-hidden /> : <Download size={18} aria-hidden />}
        </span>
        <div className="update-text">
          <strong>{t('updateAvailable', { version })}</strong>
          <span className="muted small">
            {phase.status === 'error'
              ? t('updateError')
              : done
                ? t('updateInstalling')
                : downloading
                  ? t('updateDownloading', { percent: percent === null ? '' : `${percent}%` })
                  : t('updateBody')}
          </span>
        </div>
      </div>

      {downloading ? (
        <div
          className={percent === null ? 'progress progress-indeterminate' : 'progress'}
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percent ?? undefined}
        >
          <span style={percent === null ? undefined : { width: `${percent}%` }} />
        </div>
      ) : (
        <div className="update-actions">
          <button className="btn btn-sm btn-ghost" onClick={() => setVersion(null)}>
            {t('updateLater')}
          </button>
          <button className="btn btn-sm btn-primary" onClick={install}>
            <Download size={14} aria-hidden />
            {phase.status === 'error' ? t('retry') : t('updateInstall')}
          </button>
        </div>
      )}
    </aside>
  )
}
