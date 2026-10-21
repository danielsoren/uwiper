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

const LIGHT = 'size-3 rounded-full group-hover:brightness-85';

/** macOS-style traffic lights for the top of the sidebar. */
export function TrafficLights() {
  return (
    <div className="group flex gap-2 px-2 pb-3 pt-1.5" data-tauri-drag-region>
      <button type="button" aria-label="Close" onClick={() => windowAction('close')} className={cn(LIGHT, 'bg-[#ff5f57]')} />
      <button type="button" aria-label="Minimize" onClick={() => windowAction('minimize')} className={cn(LIGHT, 'bg-[#febc2e]')} />
      <button type="button" aria-label="Maximize" onClick={() => windowAction('toggleMaximize')} className={cn(LIGHT, 'bg-[#28c840]')} />
    </div>
  );
}
