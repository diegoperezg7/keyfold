import { useCallback, useLayoutEffect, useRef, useState } from 'react'

export interface SegmentedOption<T extends string> {
  id: T
  label: string
  count?: number
}

/**
 * Segmented control with a glass indicator that slides to the option under the cursor and
 * settles on the selected one when the cursor leaves. The selected option also keeps its own
 * text style, so the choice stays readable while the indicator previews another one.
 */
export function Segmented<T extends string>(props: {
  options: SegmentedOption<T>[]
  value: T
  onChange: (value: T) => void
  label: string
}) {
  const { options, value, onChange, label } = props
  const track = useRef<HTMLDivElement>(null)
  const [hovered, setHovered] = useState<T | null>(null)
  const [thumb, setThumb] = useState<{ x: number; width: number } | null>(null)
  const target = hovered ?? value

  const measure = useCallback(() => {
    const button = track.current?.querySelector<HTMLButtonElement>(`[data-id="${target}"]`)
    if (button) setThumb({ x: button.offsetLeft, width: button.offsetWidth })
  }, [target])

  // Re-measure when the target moves, and when labels or counts change the widths.
  useLayoutEffect(measure, [measure, options])
  useLayoutEffect(() => {
    const element = track.current
    if (!element || typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    return () => observer.disconnect()
  }, [measure])

  return (
    <div className="segmented" role="tablist" aria-label={label} ref={track} onMouseLeave={() => setHovered(null)}>
      {thumb && (
        <span
          className={hovered && hovered !== value ? 'segmented-thumb previewing' : 'segmented-thumb'}
          style={{ transform: `translateX(${thumb.x}px)`, width: thumb.width }}
          aria-hidden
        />
      )}
      {options.map((option) => (
        <button
          key={option.id}
          data-id={option.id}
          role="tab"
          aria-selected={value === option.id}
          className={value === option.id ? 'active' : ''}
          onMouseEnter={() => setHovered(option.id)}
          onFocus={() => setHovered(option.id)}
          onBlur={() => setHovered(null)}
          onClick={() => onChange(option.id)}
        >
          {option.label}
          {option.count !== undefined && <span className="count">{option.count}</span>}
        </button>
      ))}
    </div>
  )
}
