import type { ReactNode } from 'react';
import { cn } from '../lib/cn';

/** A card that holds rows separated by hairlines. */
export function ListCard({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={cn('overflow-hidden rounded-card border border-line-2 bg-fill [&>*+*]:border-t [&>*+*]:border-line', className)}>
      {children}
    </div>
  );
}

const ROW = 'flex w-full items-center gap-3.5 px-4 py-2.5 text-left';

/** One row: leading tile, title and subtitle, then trailing content. */
export function ListRow({
  leading,
  title,
  subtitle,
  children,
  onClick,
  expanded,
  tone,
}: {
  leading?: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  children?: ReactNode;
  onClick?: () => void;
  expanded?: boolean;
  tone?: 'problem';
}) {
  const body = (
    <>
      {leading}
      <span className="flex min-w-0 flex-[1_1_200px] flex-col gap-0.5">
        <span className="truncate text-label font-semibold">{title}</span>
        {subtitle && <span className="text-small text-fg-3">{subtitle}</span>}
      </span>
      {children}
    </>
  );
  const rowClass = cn(ROW, 'min-h-[60px]', tone === 'problem' && 'bg-bad/4');
  return onClick ? (
    <button type="button" aria-expanded={expanded} onClick={onClick} className={cn(rowClass, 'hover:bg-hover')}>
      {body}
    </button>
  ) : (
    <div className={rowClass}>{body}</div>
  );
}
