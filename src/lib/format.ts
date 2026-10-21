export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '–';
  if (bytes <= 0) return 'Empty';
  const kb = bytes / 1024;
  if (kb < 1024) return `${Math.max(1, Math.round(kb))} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${Math.round(mb)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

export function plural(n: number, one: string, many: string): string {
  return `${n.toLocaleString('en-US')} ${n === 1 ? one : many}`;
}

export function formatDate(iso: string | null): string {
  if (!iso) return '–';
  const d = new Date(`${iso}T00:00:00`);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
}

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace('#', '').slice(0, 6), 16) || 0;
  return [n >> 16, (n >> 8) & 255, n & 255];
}

/** Mixes a color toward white; used for accent-tinted icons and links. */
export function tint(hex: string, amount: number): string {
  const [r, g, b] = rgb(hex).map((c) => Math.round(c + (255 - c) * amount));
  return `rgb(${r}, ${g}, ${b})`;
}

export function alpha(hex: string, a: number): string {
  const [r, g, b] = rgb(hex);
  return `rgba(${r}, ${g}, ${b}, ${a})`;
}

const TILE_COLORS = [
  '#B3261E', '#1D63ED', '#2A475E', '#4E7A0E', '#00727D', '#4752C4', '#3A3A44',
  '#1A7F43', '#611F69', '#127A4E', '#1F86BA', '#4F7D23', '#B4520C', '#7A3E9D',
];

/** A stable color for an app's letter tile. */
export function tileColor(name: string): string {
  let h = 0;
  for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return TILE_COLORS[h % TILE_COLORS.length];
}

export function initial(name: string): string {
  const ch = name.trim().match(/[\p{L}\p{N}]/u);
  return ch ? ch[0].toUpperCase() : '?';
}
