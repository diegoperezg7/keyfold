import { listen } from '@tauri-apps/api/event'
import { AlertCircle, ArrowDown, ArrowUp, CheckCircle2, Eye, FolderPlus, FolderSearch, KeyRound, Plus, RefreshCw, RotateCw, Search, Share2, ShieldCheck, Trash2, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState, type CSSProperties, type ReactNode } from 'react'
import { api, boot, type Notice, type SlotInfo } from '../api'
import { AssignDialog } from '../components/AssignDialog'
import { DeleteDialog } from '../components/DeleteDialog'
import { Logo } from '../components/Logo'
import { NewKeyDialog } from '../components/NewKeyDialog'
import { ImportEnvDialog } from '../components/ImportEnvDialog'
import { ShareDialog } from '../components/ShareDialog'
import { ProjectFolders } from '../components/ProjectFolders'
import { ProjectSelect } from '../components/ProjectSelect'
import { RevealDialog } from '../components/RevealDialog'
import { Segmented } from '../components/Segmented'
import { ProjectChips } from '../components/ProjectChips'
import { StatusPill } from '../components/StatusPill'
import { UpdatedDialog } from '../components/UpdatedDialog'
import { UpdateNotice } from '../components/UpdateNotice'
import { useT, type Key } from '../i18n'
import { counts, inProject, knownProjects, NO_PROJECT, visibleKeys, type SortKey, type StatusFilter } from '../logic'
import { useKeys } from '../stores/keys'

const FILTERS: { id: StatusFilter; label: Key }[] = [
  { id: 'all', label: 'filterAll' },
  { id: 'stored', label: 'filterStored' },
  { id: 'empty', label: 'filterEmpty' },
]

export function KeysView() {
  const t = useT()
  const { keys, loaded, loadFailed, search, filter, sort, project, load, setSearch, setFilter, setProject, toggleSort } = useKeys()
  const [assigning, setAssigning] = useState<SlotInfo | null>(null)
  const [revealing, setRevealing] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)
  /** Keys to share: one from its row, or [] for every key with a value. */
  const [sharing, setSharing] = useState<string[] | null>(null)
  const [importingEnv, setImportingEnv] = useState(false)
  const [creating, setCreating] = useState(boot.action === 'new')
  const [notice, setNotice] = useState<Notice | null>(boot.notice ?? null)

  // Menu bar: "New key…" and the result of "Connect agent".
  useEffect(() => {
    const stops = [
      listen('open-new-key', () => setCreating(true)),
      listen<Notice>('notice', (e) => setNotice(e.payload)),
      listen<string>('key-saved', (e) => setNotice({ ok: true, text: t('savedNotice', { name: e.payload }) })),
    ]
    return () => stops.forEach((stop) => stop.then((f) => f()))
  }, [])
  useEffect(() => {
    if (!notice) return
    const timer = setTimeout(() => setNotice(null), 8000)
    return () => clearTimeout(timer)
  }, [notice])
  const closeReveal = useCallback(() => setRevealing(null), [])
  const projects = useMemo(() => knownProjects(keys), [keys])
  // A project that no longer has keys (renamed, deleted) falls back to all.
  const activeProject = project === null || project === NO_PROJECT || projects.includes(project) ? project : null
  // Status counts follow the chosen project, so the selector tells what the list will show.
  const total = counts(keys.filter((k) => inProject(k, activeProject)))
  const rows = useMemo(
    () => visibleKeys(keys, search, filter, sort, activeProject),
    [keys, search, filter, sort, activeProject],
  )

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

  // The subtitle describes the whole vault, whatever the filters.
  const vault = counts(keys)
  const summary = [
    t(vault.all === 1 ? 'keysOne' : 'keysOther', { n: vault.all }),
    t(vault.stored === 1 ? 'storedOne' : 'storedOther', { n: vault.stored }),
    t(vault.empty === 1 ? 'emptyOne' : 'emptyOther', { n: vault.empty }),
  ].join(' · ')

  return (
    <div className="app">
      <div className="content">
      {/* Glass header: the list scrolls under it. The top row is the window's drag area. */}
      <header className="header">
      <div className="topbar" data-tauri-drag-region>
        <div className="brand" data-tauri-drag-region>
          <Logo size={30} />
          <div data-tauri-drag-region>
            <h1 data-tauri-drag-region>Keyfold</h1>
            <p className="muted small" data-tauri-drag-region>
              {summary}
            </p>
          </div>
        </div>
        <div className="topbar-actions">
          <button className="btn btn-icon btn-ghost" onClick={() => load()} title={t('refresh')} aria-label={t('refresh')}>
            <RefreshCw size={16} aria-hidden />
          </button>
          <button className="btn btn-icon btn-ghost" onClick={() => setImportingEnv(true)} title={t('envImport')} aria-label={t('envImport')}>
            <FolderSearch size={16} aria-hidden />
          </button>
          <button className="btn btn-icon btn-ghost" onClick={() => setSharing([])} title={t('shareTitle')} aria-label={t('shareTitle')}>
            <Share2 size={16} aria-hidden />
          </button>
          <button className="btn btn-primary" onClick={() => setCreating(true)}>
            <Plus size={16} aria-hidden />
            {t('newKey')}
          </button>
        </div>
      </div>

      {notice && (
        <div className={notice.ok ? 'notice' : 'notice notice-error'} role="status">
          {notice.ok ? <CheckCircle2 size={16} aria-hidden /> : <AlertCircle size={16} aria-hidden />}
          <span>{notice.text}</span>
          <button className="search-clear" onClick={() => setNotice(null)} aria-label={t('close')}>
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
            <button className="search-clear" onClick={() => setSearch('')} aria-label={t('clearSearch')}>
              <X size={14} aria-hidden />
            </button>
          )}
        </label>
        <Segmented
          label={t('colStatus')}
          value={filter}
          onChange={setFilter}
          options={FILTERS.map((f) => ({ id: f.id, label: t(f.label), count: total[f.id] }))}
        />
        <ProjectSelect projects={projects} value={activeProject} onChange={setProject} />
      </div>
      {activeProject !== null && activeProject !== NO_PROJECT && <ProjectFolders project={activeProject} />}
      </header>

      <main className="list">
        {loadFailed ? (
          <Empty title={t('loadError')} />
        ) : !loaded ? null : keys.length === 0 ? (
          <Empty title={t('emptyTitle')} body={t('emptyBody')}>
            <button className="btn btn-primary" onClick={() => setCreating(true)}>
              <Plus size={16} aria-hidden />
              {t('newKey')}
            </button>
          </Empty>
        ) : rows.length === 0 ? (
          <Empty title={t('noResultsTitle')} body={t('noResultsBody')} />
        ) : (
          <div className="table" role="table">
            <div className="row head" role="row">
              <SortHeader label={t('colName')} sortKey="name" sort={sort} onSort={toggleSort} />
              <SortHeader label={t('colStatus')} sortKey="status" sort={sort} onSort={toggleSort} />
              <span className="cell-projects" role="columnheader">
                {t('colProjects')}
              </span>
              <span className="cell-actions" role="columnheader" />
            </div>
            {rows.map((slot, i) => (
              <div className="row" role="row" key={slot.name} style={{ '--i': i } as CSSProperties}>
                <span className="name cell-name" role="cell">
                  <KeyRound size={15} aria-hidden />
                  <code title={slot.name}>{slot.name}</code>
                </span>
                <span className="cell-status" role="cell">
                  <StatusPill hasValue={slot.hasValue} />
                </span>
                <span className="cell-projects" role="cell">
                  <ProjectChips projects={slot.projects} />
                </span>
                {/* Secondary actions as quiet icons; the main one (add or reveal) at the edge. */}
                <span className="actions cell-actions" role="cell">
                  <IconButton label={t('deleteKey')} onClick={() => setDeleting(slot.name)}>
                    <Trash2 size={15} aria-hidden />
                  </IconButton>
                  {slot.hasValue && (
                    <>
                      <IconButton label={t('share')} onClick={() => setSharing([slot.name])}>
                        <Share2 size={15} aria-hidden />
                      </IconButton>
                      <IconButton label={t('changeValue')} onClick={() => api.openPrompt(slot.name)}>
                        <RotateCw size={15} aria-hidden />
                      </IconButton>
                    </>
                  )}
                  {/* Always next to the main button, so it sits in the same place in every row. */}
                  <IconButton label={t('assignProject')} onClick={() => setAssigning(slot)}>
                    <FolderPlus size={15} aria-hidden />
                  </IconButton>
                  {slot.hasValue ? (
                    <>
                      <button className="btn btn-sm" onClick={() => setRevealing(slot.name)} title={t('revealValue')}>
                        <Eye size={14} aria-hidden />
                        {t('show')}
                      </button>
                    </>
                  ) : (
                    <button className="btn btn-sm btn-primary" onClick={() => api.openPrompt(slot.name)}>
                      <Plus size={14} aria-hidden />
                      {t('addValue')}
                    </button>
                  )}
                </span>
              </div>
            ))}
          </div>
        )}
      </main>
      </div>

      <footer className="footer">
        <ShieldCheck size={14} aria-hidden />
        <span>{t('footerNote')}</span>
      </footer>

      {assigning && <AssignDialog slot={assigning} onClose={() => setAssigning(null)} />}
      {creating && <NewKeyDialog onClose={() => setCreating(false)} />}
      {sharing && <ShareDialog names={sharing} onClose={() => setSharing(null)} />}
      {importingEnv && <ImportEnvDialog onClose={() => setImportingEnv(false)} />}
      {revealing && <RevealDialog name={revealing} onClose={closeReveal} />}
      {deleting && (
        <DeleteDialog
          name={deleting}
          onClose={() => setDeleting(null)}
          onDeleted={() => setNotice({ ok: true, text: t('deletedNotice', { name: deleting }) })}
        />
      )}
      <UpdateNotice />
      <UpdatedDialog />
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
      className={`sort cell-${props.sortKey} ${active ? 'sort-active' : ''}`}
      onClick={() => props.onSort(props.sortKey)}
      aria-sort={active ? (props.sort.dir === 'asc' ? 'ascending' : 'descending') : 'none'}
    >
      {props.label}
      {active && <Arrow size={12} aria-hidden />}
    </button>
  )
}

function IconButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button className="btn btn-sm btn-icon btn-ghost" onClick={onClick} title={label} aria-label={label}>
      {children}
    </button>
  )
}

function Empty({ title, body, children }: { title: string; body?: string; children?: ReactNode }) {
  return (
    <div className="empty">
      <Logo size={64} />
      <h2>{title}</h2>
      {body && <p className="muted">{body}</p>}
      {children}
    </div>
  )
}
