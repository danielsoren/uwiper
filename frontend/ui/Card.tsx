import type { ReactNode } from 'react';
import { cn } from '../lib/cn';

/** The translucent rounded panel everything sits on. */
export function Card({ children, className, padded }: { children: ReactNode; className?: string; padded?: boolean }) {
  return <section className={cn('rounded-card border border-line-2 bg-fill', padded && 'p-[18px]', className)}>{children}</section>;
}

/** Uppercase caption above a group of cards or rows. */
export function GroupTitle({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  return (
    <h2 className="mx-1 mb-2 mt-1 flex justify-between gap-3 text-[11.5px] font-semibold uppercase tracking-[.05em] text-fg-3">
      <span>{children}</span>
      {aside && <span className="font-medium normal-case tracking-normal">{aside}</span>}
    </h2>
  );
}
