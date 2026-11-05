import { ChevronDown, FolderOpen } from 'lucide-react'
import { useT } from '../i18n'
import { NO_PROJECT, type ProjectFilter } from '../logic'

const ALL = ''

/** Pick one project (or keys without one). A native select: the menu is the system's own. */
export function ProjectSelect(props: {
  projects: string[]
  value: ProjectFilter
  onChange: (project: ProjectFilter) => void
}) {
  const t = useT()
  const { projects, value, onChange } = props
  const active = value !== null
  return (
    <label className={active ? 'select select-active' : 'select'}>
      <FolderOpen size={15} aria-hidden />
      <select
        aria-label={t('projectFilter')}
        value={value ?? ALL}
        onChange={(e) => onChange(e.target.value === ALL ? null : e.target.value)}
      >
        <option value={ALL}>{t('allProjects')}</option>
        {projects.map((p) => (
          <option key={p} value={p}>
            {p}
          </option>
        ))}
        <option value={NO_PROJECT}>{t('noProject')}</option>
      </select>
      <ChevronDown size={14} aria-hidden />
    </label>
  )
}
