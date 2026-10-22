import { windowAction } from '../../lib/api';
import { cn } from '../../lib/cn';

const CAPTION = 'grid h-8 w-[46px] place-items-center text-[#d9d9de] [&_svg]:fill-none [&_svg]:stroke-current';

/** Windows 11–style caption buttons for the top-right corner. */
export function CaptionButtons() {
  return (
    <div className="absolute right-0 top-0 z-10 flex">
      <button type="button" aria-label="Minimize" onClick={() => windowAction('minimize')} className={cn(CAPTION, 'hover:bg-white/8')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5h10" />
        </svg>
      </button>
      <button type="button" aria-label="Maximize" onClick={() => windowAction('toggleMaximize')} className={cn(CAPTION, 'hover:bg-white/8')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M.5 .5h9v9h-9z" />
        </svg>
      </button>
      <button type="button" aria-label="Close" onClick={() => windowAction('close')} className={cn(CAPTION, 'hover:bg-[#c42b1c] hover:text-white')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 0l10 10M10 0L0 10" />
        </svg>
      </button>
    </div>
  );
}

const LIGHTS = [
  { label: 'Close', action: 'close', color: 'bg-[#ff5f57]', glyph: 'M4 4l4 4M8 4l-4 4' },
  { label: 'Minimize', action: 'minimize', color: 'bg-[#febc2e]', glyph: 'M3.5 6h5' },
  { label: 'Maximize', action: 'toggleMaximize', color: 'bg-[#28c840]', glyph: 'M6 3.5v5M3.5 6h5' },
] as const;

/** macOS-style traffic lights for the top of the sidebar. Each light reacts only to its own hover. */
export function TrafficLights() {
  return (
    <div className="flex gap-2 px-2 pb-3 pt-1.5" data-tauri-drag-region>
      {LIGHTS.map((l) => (
        <button
          key={l.action}
          type="button"
          aria-label={l.label}
          onClick={() => windowAction(l.action)}
          className={cn('group/light grid size-3 place-items-center rounded-full', l.color)}
        >
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true" className="opacity-0 transition-opacity group-hover/light:opacity-100">
            <path d={l.glyph} stroke="rgb(0 0 0 / 0.55)" strokeWidth="1.3" strokeLinecap="round" fill="none" />
          </svg>
        </button>
      ))}
    </div>
  );
}
