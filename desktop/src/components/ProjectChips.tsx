import { useT } from '../i18n'

export function ProjectChips({ projects }: { projects: string[] }) {
  const t = useT()
  if (projects.length === 0) return <span className="muted">{t('noProject')}</span>
  return (
    <span className="chips">
      {projects.map((p) => (
        <span key={p} className="chip">
          {p}
        </span>
      ))}
    </span>
  )
}
