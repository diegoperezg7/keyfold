import { listen } from '@tauri-apps/api/event'
import { AlertCircle, ArrowDown, ArrowUp, CheckCircle2, Eye, FolderPlus, KeyRound, Plus, RefreshCw, RotateCw, Search, ShieldCheck, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { api, boot, type Notice, type SlotInfo } from '../api'
import { AssignDialog } from '../components/AssignDialog'
import { Logo } from '../components/Logo'
import { NewKeyDialog } from '../components/NewKeyDialog'
import { RevealDialog } from '../components/RevealDialog'
import { ProjectChips } from '../components/ProjectChips'
import { StatusPill } from '../components/StatusPill'
import { useT, type Key } from '../i18n'
import { counts, visibleKeys, type SortKey, type StatusFilter } from '../logic'
import { useKeys } from '../stores/keys'

const FILTERS: { id: StatusFilter; label: Key }[] = [
  { id: 'all', label: 'filterAll' },
  { id: 'stored', label: 'filterStored' },
  { id: 'empty', label: 'filterEmpty' },
]

export function KeysView() {
  const t = useT()
  const { keys, loaded, loadFailed, search, filter, sort, load, setSearch, setFilter, toggleSort } = useKeys()
  const [assigning, setAssigning] = useState<SlotInfo | null>(null)
  const [revealing, setRevealing] = useState<string | null>(null)
  const [creating, setCreating] = useState(boot.action === 'new')
  const [notice, setNotice] = useState<Notice | null>(boot.notice ?? null)

  // Menu bar: "New key…" and the result of "Connect agent".
  useEffect(() => {
    const stops = [
      listen('open-new-key', () => setCreating(true)),
      listen<Notice>('notice', (e) => setNotice(e.payload)),
    ]
    return () => stops.forEach((stop) => stop.then((f) => f()))
  }, [])
  useEffect(() => {
    if (!notice) return
    const timer = setTimeout(() => setNotice(null), 8000)
    return () => clearTimeout(timer)
  }, [notice])
  const closeReveal = useCallback(() => setRevealing(null), [])
  const total = counts(keys)
  const rows = useMemo(() => visibleKeys(keys, search, filter, sort), [keys, search, filter, sort])

  useEffect(() => {
    load()
    // Reload when a paste window saves, and when coming back to the app
    // (an agent may have created slots over MCP in the meantime).
    const unlisten = listen('keys-changed', () => load())
    const onFocus = () => load()
    window.addEventListener('focus', onFocus)
    return () => {
      unlisten.then((stop) => stop())
      window.removeEventListener('focus', onFocus)
    }
  }, [load])

  const summary = [
    t(total.all === 1 ? 'keysOne' : 'keysOther', { n: total.all }),
    t(total.stored === 1 ? 'storedOne' : 'storedOther', { n: total.stored }),
    t(total.empty === 1 ? 'emptyOne' : 'emptyOther', { n: total.empty }),
  ].join(' · ')

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <Logo size={40} />
          <div>
            <h1>Keyfold</h1>
            <p className="muted small">{summary}</p>
          </div>
        </div>
        <div className="topbar-actions">
          <button className="btn btn-icon" onClick={() => load()} title={t('refresh')} aria-label={t('refresh')}>
            <RefreshCw size={16} aria-hidden />
          </button>
          <button className="btn btn-primary" onClick={() => setCreating(true)}>
            <Plus size={16} aria-hidden />
            {t('newKey')}
          </button>
        </div>
      </header>

      {notice && (
        <div className={notice.ok ? 'notice' : 'notice notice-error'} role="status">
          {notice.ok ? <CheckCircle2 size={16} aria-hidden /> : <AlertCircle size={16} aria-hidden />}
          <span>{notice.text}</span>
          <button className="search-clear" onClick={() => setNotice(null)} aria-label="Close">
            <X size={14} aria-hidden />
          </button>
        </div>
      )}

      <div className="toolbar">
        <label className="search">
          <Search size={16} aria-hidden />
          <input
            value={search}
            placeholder={t('searchPlaceholder')}
            spellCheck={false}
            onChange={(e) => setSearch(e.target.value)}
          />
          {search && (
            <button className="search-clear" onClick={() => setSearch('')} aria-label="Clear">
              <X size={14} aria-hidden />
            </button>
          )}
        </label>
        <div className="segmented" role="tablist">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              role="tab"
              aria-selected={filter === f.id}
              className={filter === f.id ? 'active' : ''}
              onClick={() => setFilter(f.id)}
            >
              {t(f.label)}
              <span className="count">{total[f.id]}</span>
            </button>
          ))}
        </div>
      </div>

      <main className="content">
        {loadFailed ? (
          <Empty title={t('loadError')} />
        ) : !loaded ? null : keys.length === 0 ? (
          <Empty title={t('emptyTitle')} body={t('emptyBody')} />
        ) : rows.length === 0 ? (
          <Empty title={t('noResultsTitle')} body={t('noResultsBody')} />
        ) : (
          <div className="table" role="table">
            <div className="row head" role="row">
              <SortHeader label={t('colName')} sortKey="name" sort={sort} onSort={toggleSort} />
              <SortHeader label={t('colStatus')} sortKey="status" sort={sort} onSort={toggleSort} />
              <span role="columnheader">{t('colProjects')}</span>
              <span role="columnheader" />
            </div>
            {rows.map((slot) => (
              <div className="row" role="row" key={slot.name}>
                <span className="name" role="cell">
                  <KeyRound size={15} aria-hidden />
                  <code>{slot.name}</code>
                </span>
                <span role="cell">
                  <StatusPill hasValue={slot.hasValue} />
                </span>
                <span role="cell">
                  <ProjectChips projects={slot.projects} />
                </span>
                <span className="actions" role="cell">
                  {slot.hasValue ? (
                    <button className="btn btn-sm" onClick={() => api.openPrompt(slot.name)}>
                      <RotateCw size={14} aria-hidden />
                      {t('changeValue')}
                    </button>
                  ) : (
                    <button className="btn btn-sm btn-primary" onClick={() => api.openPrompt(slot.name)}>
                      <Plus size={14} aria-hidden />
                      {t('addValue')}
                    </button>
                  )}
                  <button className="btn btn-sm" onClick={() => setAssigning(slot)}>
                    <FolderPlus size={14} aria-hidden />
                    {t('assignProject')}
                  </button>
                  {slot.hasValue ? (
                    <button
                      className="btn btn-sm btn-icon"
                      onClick={() => setRevealing(slot.name)}
                      title={t('revealValue')}
                      aria-label={t('revealValue')}
                    >
                      <Eye size={14} aria-hidden />
                    </button>
                  ) : (
                    <span className="btn-icon-spacer" aria-hidden />
                  )}
                </span>
              </div>
            ))}
          </div>
        )}
      </main>

      <footer className="footer">
        <ShieldCheck size={14} aria-hidden />
        <span>{t('footerNote')}</span>
      </footer>

      {assigning && <AssignDialog slot={assigning} onClose={() => setAssigning(null)} />}
      {creating && <NewKeyDialog onClose={() => setCreating(false)} />}
      {revealing && <RevealDialog name={revealing} onClose={closeReveal} />}
    </div>
  )
}

function SortHeader(props: {
  label: string
  sortKey: SortKey
  sort: { key: SortKey; dir: 'asc' | 'desc' }
  onSort: (key: SortKey) => void
}) {
  const active = props.sort.key === props.sortKey
  const Arrow = props.sort.dir === 'asc' ? ArrowUp : ArrowDown
  return (
    <button
      role="columnheader"
      className={`sort ${active ? 'sort-active' : ''}`}
      onClick={() => props.onSort(props.sortKey)}
      aria-sort={active ? (props.sort.dir === 'asc' ? 'ascending' : 'descending') : 'none'}
    >
      {props.label}
      {active && <Arrow size={12} aria-hidden />}
    </button>
  )
}

function Empty({ title, body }: { title: string; body?: string }) {
  return (
    <div className="empty">
      <Logo size={56} />
      <h2>{title}</h2>
      {body && <p className="muted">{body}</p>}
    </div>
  )
}
