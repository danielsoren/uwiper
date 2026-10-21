import { useEffect, useState } from 'react';
import { cn } from '../../lib/cn';
import { useAppStore } from '../../store/useAppStore';
import { Spinner } from '../../ui';
import { Logo } from '../brand/Logo';

/** Covers a slow start. Settings usually load in a few milliseconds, so it
 *  stays invisible unless the start takes longer than a moment. */
export function Splash() {
  const accent = useAppStore((s) => s.settings?.appearance.accent ?? '#0071E3');
  const [shown, setShown] = useState(false);
  useEffect(() => {
    const timer = setTimeout(() => setShown(true), 300);
    return () => clearTimeout(timer);
  }, []);

  return (
    <div
      data-tauri-drag-region
      className={cn('flex flex-1 flex-col items-center justify-center gap-5 bg-main transition-opacity duration-300', !shown && 'opacity-0')}
    >
      <Logo size={64} color={accent} />
      <div className="flex items-center gap-2.5 text-meta text-fg-3">
        <Spinner size={14} />
        Getting things ready…
      </div>
    </div>
  );
}
