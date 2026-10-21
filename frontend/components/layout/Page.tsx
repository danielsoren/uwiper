import type { ReactNode } from 'react';

/** Page title row with actions on the right. Doubles as the window's drag area. */
export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header data-tauri-drag-region className="flex min-h-[60px] flex-wrap items-center gap-3 px-7 pb-3.5 pt-1">
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
      {children && <div className="ml-auto flex items-center gap-2">{children}</div>}
    </header>
  );
}

/** The scrolling area under a page header. */
export function PageBody({ children }: { children: ReactNode }) {
  return <div className="scroll-thin flex min-h-0 flex-1 flex-col gap-4 overflow-auto px-7 pb-7 pt-1 [&>*]:shrink-0">{children}</div>;
}

/** A row of controls between the header and the content. */
export function Toolbar({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-2.5 px-7 pb-3.5">{children}</div>;
}
