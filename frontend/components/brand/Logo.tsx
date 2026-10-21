// The UWiper mark: a field of dots (traces) with one clean wiper sweep.
// Geometry lives on a 30×30 grid; see assets/logo.svg for the app icon.

const DOTS: [number, number][] = [
  [6, 2.4], [9.6, 2.4], [13.2, 2.4], [16.8, 2.4], [20.4, 2.4], [24, 2.4],
  [2.4, 6], [6, 6], [9.6, 6], [13.2, 6], [16.8, 6], [20.4, 6], [24, 6], [27.6, 6],
  [2.4, 9.6], [6, 9.6], [24, 9.6], [27.6, 9.6],
  [2.4, 13.2], [27.6, 13.2],
  [2.4, 16.8], [27.6, 16.8],
  [2.4, 20.4], [6, 20.4], [9.6, 20.4], [20.4, 20.4], [24, 20.4], [27.6, 20.4],
  [2.4, 24], [6, 24], [9.6, 24], [13.2, 24], [16.8, 24], [20.4, 24], [24, 24], [27.6, 24],
  [6, 27.6], [9.6, 27.6], [13.2, 27.6], [16.8, 27.6], [20.4, 27.6], [24, 27.6],
];

export function Logo({ size = 30, color }: { size?: number; color: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 30 30" aria-hidden="true" style={{ flex: 'none' }}>
      <rect width="30" height="30" rx="8" fill={color} />
      <path d="M26.09 18.04A11.2 11.2 0 0 0 3.91 18.04L11.83 19.15A3.2 3.2 0 0 1 18.17 19.15Z" fill="#fff" fillOpacity={0.14} />
      <g fill="#fff" fillOpacity={0.45}>
        {DOTS.map(([cx, cy]) => (
          <circle key={`${cx}-${cy}`} cx={cx} cy={cy} r={0.75} />
        ))}
      </g>
      <path d="M15 19.6V15.2" fill="none" stroke="#fff" strokeWidth={1.3} strokeLinecap="round" />
      <path d="M15 15.2V9" fill="none" stroke="#fff" strokeWidth={2.5} strokeLinecap="round" />
      <circle cx="15" cy="19.6" r="2.1" fill="#fff" />
    </svg>
  );
}
