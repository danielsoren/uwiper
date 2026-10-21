import type { ReactNode } from 'react';
import { cn } from '../../lib/cn';
import { GroupTitle, Icon } from '../../ui';

/** A titled group of settings rows, like a System Settings pane. */
export function SettingsSection({ title, note, danger, children }: { title: string; note?: string; danger?: boolean; children: ReactNode }) {
  return (
    <section>
      <GroupTitle>{title}</GroupTitle>
      {note && <p className="mx-1 -mt-0.5 mb-2.5 text-small leading-normal text-fg-4">{note}</p>}
      <div
        className={cn(
          'overflow-hidden rounded-card border bg-fill [&>*+*]:border-t [&>*+*]:border-line',
          danger ? 'border-bad/22' : 'border-line-2',
        )}
      >
        {children}
      </div>
    </section>
  );
}

/** One setting: label and explanation on the left, its control on the right. */
export function SettingsRow({
  label,
  sub,
  mono,
  control,
  children,
  warn,
}: {
  label: string;
  sub?: ReactNode;
  mono?: string;
  control?: ReactNode;
  children?: ReactNode;
  warn?: boolean;
}) {
  return (
    <div className="flex min-h-14 flex-wrap items-center gap-x-4 gap-y-2.5 px-4 py-2.5">
      <div className="min-w-0 flex-[1_1_260px]">
        <div className="text-label">{label}</div>
        {sub && <div className={cn('mt-0.5 text-[11.5px] leading-[1.45]', warn ? 'text-warn-fg' : 'text-fg-4')}>{sub}</div>}
        {mono && <div className="mt-[5px] select-text break-all font-mono text-[11.5px] text-fg-path">{mono}</div>}
      </div>
      {control && <div className="flex flex-none items-center gap-2.5">{control}</div>}
      {children && <div className="flex-[1_1_100%]">{children}</div>}
    </div>
  );
}

/** "In UWiper folder" style marker for settings that can't be changed. */
export function Locked({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-[5px] text-[11.5px] text-fg-4">
      <Icon name="lock" size={13} stroke={2} />
      {children}
    </span>
  );
}
