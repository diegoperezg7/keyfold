import { Check, Copy, Share2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { api } from '../api'
import { useT } from '../i18n'
import { isPublicKey, parseRecipients } from '../logic'
import { Segmented } from './Segmented'

type Tab = 'send' | 'receive'
type Status = { kind: 'idle' | 'busy' } | { kind: 'done' | 'error'; text: string }

/**
 * Send: encrypt keys to other people's (or your other Macs') public keys and copy the package.
 * Receive: show this computer's public key and import a package made for it.
 * Values never reach this component: only public keys and encrypted packages do.
 */
export function ShareDialog({ names, onClose }: { names: string[]; onClose: () => void }) {
  const t = useT()
  const [tab, setTab] = useState<Tab>('send')

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && onClose()
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="share-title">
        <div className="dialog-head">
          <span className="icon-badge">
            <Share2 size={18} aria-hidden />
          </span>
          <h2 id="share-title">{t('shareTitle')}</h2>
        </div>
        <Segmented
          label={t('shareTitle')}
          value={tab}
          onChange={setTab}
          options={[
            { id: 'send', label: t('shareSend') },
            { id: 'receive', label: t('shareReceive') },
          ]}
        />
        {tab === 'send' ? <Send names={names} onClose={onClose} /> : <Receive onClose={onClose} />}
      </div>
    </div>
  )
}

function Send({ names, onClose }: { names: string[]; onClose: () => void }) {
  const t = useT()
  const [to, setTo] = useState('')
  const [status, setStatus] = useState<Status>({ kind: 'idle' })
  const recipients = parseRecipients(to)
  const valid = recipients.length > 0 && recipients.every(isPublicKey)

  async function encrypt() {
    setStatus({ kind: 'busy' })
    try {
      const pkg = await api.shareKeys(names, recipients)
      await navigator.clipboard.writeText(pkg)
      setStatus({ kind: 'done', text: t('shareCopied') })
    } catch (error) {
      setStatus({ kind: 'error', text: String(error) === 'locked' ? t('shareCancelled') : String(error) })
    }
  }

  return (
    <>
      <p className="muted">{names.length === 1 ? t('shareWhatOne', { name: names[0] }) : t('shareWhatAll')}</p>
      <label className="field">
        <span className="label">{t('shareTo')}</span>
        <textarea
          className="input input-area mono"
          value={to}
          rows={3}
          placeholder="age1…"
          spellCheck={false}
          autoCapitalize="off"
          autoComplete="off"
          autoFocus
          onChange={(e) => {
            setTo(e.target.value)
            setStatus({ kind: 'idle' })
          }}
        />
        <span className={to.trim() && !valid ? 'hint hint-error' : 'hint'}>
          {to.trim() && !valid ? t('shareToInvalid') : t('shareToHint')}
        </span>
      </label>
      {(status.kind === 'done' || status.kind === 'error') && (
        <p className={status.kind === 'error' ? 'hint hint-error' : 'hint'}>{status.text}</p>
      )}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onClose}>
          {t('close')}
        </button>
        <button type="button" className="btn btn-primary" disabled={!valid || status.kind === 'busy'} onClick={encrypt}>
          {status.kind === 'busy' ? t('shareEncrypting') : t('shareEncrypt')}
        </button>
      </div>
    </>
  )
}

function Receive({ onClose }: { onClose: () => void }) {
  const t = useT()
  const [key, setKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [pkg, setPkg] = useState('')
  const [status, setStatus] = useState<Status>({ kind: 'idle' })

  useEffect(() => {
    api.publicKey().then(setKey).catch((error) => setStatus({ kind: 'error', text: String(error) }))
  }, [])

  async function copyKey() {
    if (!key) return
    await navigator.clipboard.writeText(key)
    setCopied(true)
    setTimeout(() => setCopied(false), 1500)
  }

  async function importIt() {
    setStatus({ kind: 'busy' })
    try {
      const imported = await api.importPackage(pkg)
      setPkg('')
      setStatus({ kind: 'done', text: t(imported.length === 1 ? 'importDoneOne' : 'importDoneOther', { n: imported.length }) })
    } catch (error) {
      setStatus({ kind: 'error', text: String(error) })
    }
  }

  return (
    <>
      <div className="field">
        <span className="label">{t('myKey')}</span>
        <div className="field-row">
          <code className="public-key">{key ?? '…'}</code>
          <button type="button" className="btn btn-sm" onClick={copyKey} disabled={!key}>
            {copied ? <Check size={14} aria-hidden /> : <Copy size={14} aria-hidden />}
            {copied ? t('copied') : t('copy')}
          </button>
        </div>
        <span className="hint">{t('myKeyHint')}</span>
      </div>
      <label className="field">
        <span className="label">{t('importLabel')}</span>
        <textarea
          className="input input-area mono"
          value={pkg}
          rows={4}
          placeholder={t('importPlaceholder')}
          spellCheck={false}
          autoCapitalize="off"
          autoComplete="off"
          onChange={(e) => {
            setPkg(e.target.value)
            setStatus({ kind: 'idle' })
          }}
        />
      </label>
      {(status.kind === 'done' || status.kind === 'error') && (
        <p className={status.kind === 'error' ? 'hint hint-error' : 'hint'}>{status.text}</p>
      )}
      <div className="dialog-actions">
        <button type="button" className="btn" onClick={onClose}>
          {t('close')}
        </button>
        <button type="button" className="btn btn-primary" disabled={!pkg.trim() || status.kind === 'busy'} onClick={importIt}>
          {t('importAction')}
        </button>
      </div>
    </>
  )
}
