import { Folder, X } from 'lucide-react'
import { useState } from 'react'
import { useT } from '../i18n'
import { useKeys } from '../stores/keys'

/**
 * Folders linked to the chosen project on this machine (`keyfold link`). Linking needs the
 * folder path, so it happens from the CLI or the agent; here they can be seen and removed.
 * `.keyfold` files live in each repo and are not listed: the app does not scan the disk.
 */
export function ProjectFolders({ project }: { project: string }) {
  const t = useT()
  const folders = useKeys((s) => s.folders)
  const unlink = useKeys((s) => s.unlink)
  const [error, setError] = useState<string | null>(null)
  const paths = Object.entries(folders)
    .filter(([, p]) => p === project)
    .map(([path]) => path)

  async function remove(path: string) {
    setError(await unlink(path))
  }

  return (
    <div className="folders">
      <span className="label">{t('linkedFolders')}</span>
      {paths.length === 0 ? (
        <span className="muted small">{t('noLinkedFolders', { project })}</span>
      ) : (
        <span className="chips">
          {paths.map((path) => (
            <span key={path} className="chip chip-removable chip-path" title={path}>
              <Folder size={12} aria-hidden />
              <span className="chip-text">{path}</span>
              <button
                type="button"
                className="chip-remove"
                aria-label={t('unlinkFolder', { path })}
                title={t('unlinkFolder', { path })}
                onClick={() => remove(path)}
              >
                <X size={12} aria-hidden />
              </button>
            </span>
          ))}
        </span>
      )}
      {error && <span className="hint hint-error">{error}</span>}
    </div>
  )
}
