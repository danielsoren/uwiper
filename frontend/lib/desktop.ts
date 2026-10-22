import { inTauri } from './api';

/** Keys the WebView would act on like a browser: reload (which drops what's on screen), print, save, find, history. */
const CTRL_KEYS = ['r', 'p', 's', 'u', 'f', 'g', 'j', 'h'];
const PLAIN_KEYS = ['f3', 'f5', 'f7', 'browserback', 'browserforward', 'browserrefresh'];

/**
 * Makes the window behave like an app rather than a web page: no Back /
 * Refresh / Print menu, no browser shortcuts or mouse back buttons. Text
 * fields and selected text keep their menu for copy and paste. Development
 * builds keep everything, for reloading and the inspector.
 */
export function lockBrowserChrome() {
  if (!inTauri || import.meta.env.DEV) return;
  document.addEventListener(
    'contextmenu',
    (e) => {
      const editable = (e.target as Element | null)?.closest?.('input, textarea, [contenteditable="true"]');
      const selected = !!window.getSelection()?.toString();
      if (!editable && !selected) e.preventDefault();
    },
    true,
  );
  document.addEventListener(
    'keydown',
    (e) => {
      const key = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;
      const blocked =
        PLAIN_KEYS.includes(key) || (ctrl && CTRL_KEYS.includes(key)) || (e.altKey && (key === 'arrowleft' || key === 'arrowright'));
      if (blocked) e.preventDefault();
    },
    true,
  );
  // The mouse's back and forward buttons.
  for (const type of ['mouseup', 'auxclick'] as const) {
    document.addEventListener(
      type,
      (e) => {
        if (e.button === 3 || e.button === 4) e.preventDefault();
      },
      true,
    );
  }
}
