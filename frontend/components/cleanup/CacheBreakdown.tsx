import { cleanupGroupsOf } from './groups';
import { CLEANUP_GROUPS } from '../../lib/cleanupGroups';
import { cn } from '../../lib/cn';
import { formatBytes } from '../../lib/format';
import type { CleanupItem } from '../../lib/types';
import { Icon } from '../../ui';

/** A stacked bar of cache sizes by group. */
export function StackBar({ parts }: { parts: { key: string; bytes: number; color: string; label: string }[] }) {
  return (
    <div className="flex h-3 gap-[3px]" role="img" aria-label={parts.map((p) => `${p.label} ${formatBytes(p.bytes)}`).join(', ')}>
      {parts.length === 0 && <span className="flex-1 rounded-md bg-white/8" />}
      {parts.map((p) => (
        <span
          key={p.key}
          style={{ flexGrow: p.bytes || 1, background: p.color }}
          className="basis-0 rounded-[3px] first:rounded-l-md last:rounded-r-md"
        />
      ))}
    </div>
  );
}

/** Bar plus a legend; rows are clickable when `onPick` is given. */
export function CacheBreakdown({
  items,
  onlyDefault,
  inline,
  onPick,
}: {
  items: CleanupItem[];
  onlyDefault?: boolean;
  inline?: boolean;
  onPick?: () => void;
}) {
  const parts = cleanupGroupsOf(onlyDefault ? items.filter((i) => i.defaultOn) : items).map((g) => ({
    key: g.group,
    bytes: g.bytes,
    color: CLEANUP_GROUPS[g.group].color,
    label: CLEANUP_GROUPS[g.group].label,
  }));

  return (
    <div className="flex min-w-0 flex-[1_1_340px] flex-col gap-3">
      <StackBar parts={parts} />
      <div className={cn('flex', inline ? 'flex-wrap gap-x-5 gap-y-2' : 'flex-col gap-0.5')}>
        {parts.map((p) =>
          inline ? (
            <span key={p.key} className="inline-flex items-center gap-2 text-meta text-fg-2">
              <span className="size-2.5 rounded-[3px]" style={{ background: p.color }} />
              {p.label}
              <span className="text-small text-fg-3">{formatBytes(p.bytes)}</span>
            </span>
          ) : (
            <button
              key={p.key}
              type="button"
              onClick={onPick}
              className="flex min-h-11 items-center gap-3 rounded-control px-2.5 text-left hover:bg-hover"
            >
              <span className="size-2.5 flex-none rounded-[3px]" style={{ background: p.color }} />
              <span className="flex-1 font-medium">{p.label}</span>
              <span className="font-semibold tabular-nums">{formatBytes(p.bytes)}</span>
              <Icon name="chevron" size={14} className="text-fg-4" />
            </button>
          ),
        )}
      </div>
    </div>
  );
}
