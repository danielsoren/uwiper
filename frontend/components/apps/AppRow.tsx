import { cn } from '../../lib/cn';
import { formatBytes } from '../../lib/format';
import type { InstalledApp } from '../../lib/types';
import { Tag } from '../../ui';
import { AppTile } from './AppTile';

export function AppRow({ app, selected, sizeBytes, onSelect }: { app: InstalledApp; selected: boolean; sizeBytes: number | null; onSelect: () => void }) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      onClick={onSelect}
      className={cn(
        'mb-0.5 flex min-h-[54px] w-full items-center gap-3 rounded-control px-3 py-2 text-left',
        selected ? 'bg-accent text-white' : 'hover:bg-hover',
      )}
    >
      <AppTile name={app.name} />
      <span className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="truncate text-label font-semibold">{app.name}</span>
        <span className={cn('truncate text-small', selected ? 'text-white/82' : 'text-fg-3')}>{app.publisher ?? 'Unknown publisher'}</span>
      </span>
      {app.source === 'msi' && <Tag inverted={selected}>MSI</Tag>}
      {app.scope === 'user' && <Tag inverted={selected}>User</Tag>}
      <span className={cn('w-16 flex-none text-right text-meta tabular-nums', selected ? 'text-white/82' : 'text-fg-3')}>
        {formatBytes(sizeBytes)}
      </span>
    </button>
  );
}
