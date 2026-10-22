import { useEffect, useState } from 'react';
import { alpha, tint } from '../lib/format';
import { useAppStore, type Resource, type ResourceKey, type Resources } from './useAppStore';

/** Reads a resource and starts loading it the first time a page needs it. */
export function useResource<K extends ResourceKey>(key: K): Resource<Resources[K]> & { reload: () => void } {
  const resource = useAppStore((s) => s.res[key]);
  const load = useAppStore((s) => s.load);
  useEffect(() => load(key), [key, load]);
  return { ...resource, reload: () => load(key, true) };
}

/** Keeps the accent color and window material CSS variables in sync with Settings. */
export function useThemeSync() {
  const accent = useAppStore((s) => s.settings?.appearance.accent ?? '#0071E3');
  const translucent = useAppStore((s) => (s.settings?.appearance.translucent ?? true) && s.env?.material === 'mica');
  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty('--accent', accent);
    root.style.setProperty('--accent-soft', tint(accent, 0.4));
    root.style.setProperty('--accent-tile', alpha(accent, 0.2));
    root.dataset.material = translucent ? 'mica' : 'solid';
  }, [accent, translucent]);
}

/** Takes the section `go(page, focus)` asked for once `ready`. With `scroll`, also brings the element with
 * `id={`section-${focus}`}` into view. Returns the section for a short highlight. */
export function useFocus(ready: boolean, scroll = true): string | null {
  const focus = useAppStore((s) => s.focus);
  const [flash, setFlash] = useState<string | null>(null);
  useEffect(() => {
    if (!focus || !ready) return;
    useAppStore.setState({ focus: null });
    if (scroll) document.getElementById(`section-${focus}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setFlash(focus);
  }, [focus, ready, scroll]);
  useEffect(() => {
    if (!flash) return;
    const t = setTimeout(() => setFlash(null), 1600);
    return () => clearTimeout(t);
  }, [flash]);
  return flash;
}
