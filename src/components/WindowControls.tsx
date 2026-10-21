import { windowAction } from '../lib/api';

/** Windows 11–style caption buttons for the top-right corner. */
export function CaptionButtons() {
  return (
    <div className="caption-buttons">
      <button type="button" aria-label="Minimize" onClick={() => windowAction('minimize')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5h10" />
        </svg>
      </button>
      <button type="button" aria-label="Maximize" onClick={() => windowAction('toggleMaximize')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M.5 .5h9v9h-9z" />
        </svg>
      </button>
      <button type="button" aria-label="Close" className="close" onClick={() => windowAction('close')}>
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 0l10 10M10 0L0 10" />
        </svg>
      </button>
    </div>
  );
}

/** macOS-style traffic lights for the top of the sidebar. */
export function TrafficLights() {
  return (
    <div className="traffic-lights" data-tauri-drag-region>
      <button type="button" aria-label="Close" className="tl-close" onClick={() => windowAction('close')} />
      <button type="button" aria-label="Minimize" className="tl-min" onClick={() => windowAction('minimize')} />
      <button type="button" aria-label="Maximize" className="tl-max" onClick={() => windowAction('toggleMaximize')} />
    </div>
  );
}
