/** Circle and check that draw themselves (see .success-mark in styles.css). */
export function SuccessMark() {
  return (
    <svg className="success-mark" width="64" height="64" viewBox="0 0 64 64" aria-hidden>
      <circle className="success-ring" cx="32" cy="32" r="29" />
      <path className="success-check" d="M20 33 l8 8 l16 -17" />
    </svg>
  )
}
