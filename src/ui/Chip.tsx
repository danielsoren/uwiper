import type { ReactNode } from 'react';
import { cn } from '../lib/cn';
import { Icon } from './Icon';

/** A toggleable pill for multi-select options. */
export function Chip({ on, onToggle, children }: { on: boolean; onToggle: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={on}
      onClick={onToggle}
      className={cn(
        'inline-flex h-[30px] items-center gap-1.5 rounded-full border px-3 text-meta font-medium',
        on ? 'border-accent bg-accent text-white' : 'border-white/12 bg-white/5 text-fg-2',
      )}
    >
      {on && <Icon name="check" size={12} stroke={3} />}
      {children}
    </button>
  );
}

/** A round color button for picking an accent. */
export function Swatch({ color, label, on, onPick }: { color: string; label: string; on: boolean; onPick: () => void }) {
  return (
    <button
      type="button"
      aria-label={label}
      aria-pressed={on}
      onClick={onPick}
      style={{ background: color, color }}
      className={cn('size-[26px] rounded-full', on && 'shadow-[0_0_0_2px_#1c1c1f,0_0_0_4px_currentColor]')}
    />
  );
}
