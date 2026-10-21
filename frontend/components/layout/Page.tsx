import type { ReactNode } from 'react';
import { useAppStore } from '../../store/useAppStore';
import { cn } from '../../lib/cn';

/** Page title row. Doubles as the window's drag area. */
export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  // Leave room for the Windows caption buttons in the top-right corner.
  const captionButtons = useAppStore((s) => s.settings?.appearance.windowButtons !== 'macos');
  return (
    <header
      data-tauri-drag-region
      className={cn('flex min-h-[72px] flex-wrap items-center gap-3 pb-3.5 pl-7 pt-5', captionButtons ? 'pr-[156px]' : 'pr-7')}
    >
      <div className="min-w-0 flex-[1_1_260px]" data-tauri-drag-region>
        <h1 className="text-h1 font-bold tracking-[-0.02em]" data-tauri-drag-region>
          {title}
        </h1>
        {subtitle && (
          <p className="mt-[3px] text-small text-fg-3" data-tauri-drag-region>
            {subtitle}
          </p>
        )}
      </div>
      {children && <div className="flex items-center gap-2">{children}</div>}
    </header>
  );
}

/** The scrolling area under a page header. */
export function PageBody({ children, narrow }: { children: ReactNode; narrow?: boolean }) {
  return (
    <div
      className={cn(
        'scroll-thin flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-7 pb-7 pt-1 [&>*]:shrink-0',
        narrow && '[&>*]:mx-auto [&>*]:w-full [&>*]:max-w-[800px]',
      )}
    >
      {children}
    </div>
  );
}

/** A row of controls between the header and the content. */
export function Toolbar({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-2.5 px-7 pb-3.5">{children}</div>;
}
