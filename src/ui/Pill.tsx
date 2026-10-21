import type { ReactNode } from 'react';
import { cn } from '../lib/cn';

export type PillTone = 'good' | 'warn' | 'bad' | 'dim' | 'accent';

const TONE: Record<PillTone, string> = {
  good: 'bg-good/14 text-good-fg',
  warn: 'bg-warn/15 text-warn-fg',
  bad: 'bg-bad/15 text-bad-fg',
  dim: 'bg-white/8 text-[#b4b4bb]',
  accent: 'bg-accent text-white',
};

/** A small status label. */
export function Pill({ tone = 'dim', children, className }: { tone?: PillTone; children: ReactNode; className?: string }) {
  return (
    <span className={cn('flex-none whitespace-nowrap rounded-md px-[9px] py-[3px] text-micro font-semibold', TONE[tone], className)}>
      {children}
    </span>
  );
}

/** A quieter tag, e.g. "MSI" next to an app name. */
export function Tag({ children, inverted }: { children: ReactNode; inverted?: boolean }) {
  return (
    <span
      className={cn(
        'flex-none rounded-md px-[7px] py-0.5 text-micro font-semibold',
        inverted ? 'bg-white/20 text-white' : 'bg-white/8 text-fg-2',
      )}
    >
      {children}
    </span>
  );
}
