import { CircleAlert, CircleCheck } from 'lucide-react'
import { useT } from '../i18n'

export function StatusPill({ hasValue }: { hasValue: boolean }) {
  const t = useT()
  const Icon = hasValue ? CircleCheck : CircleAlert
  return (
    <span className={`pill ${hasValue ? 'pill-ok' : 'pill-warn'}`}>
      <Icon size={14} strokeWidth={2.2} aria-hidden />
      {hasValue ? t('statusStored') : t('statusEmpty')}
    </span>
  )
}
