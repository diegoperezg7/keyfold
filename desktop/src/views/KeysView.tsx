import { listen } from '@tauri-apps/api/event'
import { ArrowDown, ArrowUp, FolderPlus, KeyRound, Plus, RefreshCw, RotateCw, Search, ShieldCheck, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { api, type SlotInfo } from '../api'
import { AssignDialog } from '../components/AssignDialog'
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
          <span className="brand-mark">
            <KeyRound size={18} strokeWidth={2.2} aria-hidden />
          </span>
          <div>
            <h1>Llavero</h1>
            <p className="muted small">{summary}</p>
          </div>
        </div>
        <button className="btn btn-icon" onClick={() => load()} title={t('refresh')} aria-label={t('refresh')}>
          <RefreshCw size={16} aria-hidden />
        </button>
      </header>

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
      <span className="icon-badge icon-badge-lg">
        <KeyRound size={22} aria-hidden />
      </span>
      <h2>{title}</h2>
      {body && <p className="muted">{body}</p>}
    </div>
  )
}
