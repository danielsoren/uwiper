import type { ReactNode } from 'react';

/** Page title row. Doubles as the window's drag area. */
export function PageHeader({ title, subtitle, children }: { title: string; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header className="page-header" data-tauri-drag-region>
      <div className="page-title" data-tauri-drag-region>
        <h1 data-tauri-drag-region>{title}</h1>
        {subtitle && (
          <p className="page-subtitle" data-tauri-drag-region>
            {subtitle}
          </p>
        )}
      </div>
      {children && <div className="page-actions">{children}</div>}
    </header>
  );
}
