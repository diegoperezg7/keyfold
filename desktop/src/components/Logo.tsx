import logo from '../assets/logo.png'

/** The app icon, bundled with the UI (no remote image). */
export function Logo({ size }: { size: number }) {
  return <img className="logo" src={logo} width={size} height={size} alt="" draggable={false} />
}
