import { useState } from 'react';
import { cn } from '../../lib/cn';
import { formatDate, plural } from '../../lib/format';
import type { TraceItem, TraceView } from '../../lib/types';
import { Button, Card, Collapse, Icon, IconTile, Pill, Tag, type IconName } from '../../ui';

const KIND_ICON: Record<TraceItem['kind'], IconName> = {
  app: 'apps',
  environment: 'code',
  folder: 'folder',
  file: 'file',
  shortcut: 'link',
  registry: 'cube',
  service: 'cog',
  task: 'calendar',
  startup: 'power',
  driver: 'cpu',
};

const isoDay = (unix: number) => new Date(unix * 1000).toISOString().slice(0, 10);

/** A saved trace: what one installer added, and how much of it is still there. */
export function TraceCard({
  trace,
  onUninstall,
  onCleanUp,
  onDelete,
}: {
  trace: TraceView;
  onUninstall: () => void;
  onCleanUp: () => void;
  onDelete: () => void;
}) {
  const [open, setOpen] = useState(false);
  const left = trace.present.filter(Boolean).length;
  const status = trace.installed
    ? { tone: 'good' as const, label: 'Installed' }
    : trace.appId
      ? { tone: 'dim' as const, label: left ? 'Uninstalled' : 'Fully removed' }
      : { tone: 'dim' as const, label: 'No uninstall entry' };

  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3.5 px-4 py-3.5">
        <IconTile icon="install" tone={trace.installed ? 'accent' : 'neutral'} />
        <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex min-w-0 flex-1 items-center gap-2.5 text-left">
          <span className="min-w-0 flex-1">
            <span className="block truncate text-label font-semibold">
              {trace.name}
              {trace.version && <span className="font-normal text-fg-3"> {trace.version}</span>}
            </span>
            <span className="mt-0.5 block text-small text-fg-3">
              Tracked {formatDate(isoDay(trace.finishedAt))} · {plural(trace.items.length, 'item', 'items')}
              {!trace.installed && left > 0 && ` · ${left} still there`}
            </span>
          </span>
          <Pill tone={status.tone}>{status.label}</Pill>
          <Icon name="chevron" size={14} className={cn('flex-none text-fg-4 transition-transform', open && 'rotate-90')} />
        </button>
        {trace.installed ? (
          <Button small icon="trash" onClick={onUninstall}>
            Uninstall…
          </Button>
        ) : (
          left > 0 && (
            <Button small icon="trash" onClick={onCleanUp}>
              {trace.appId ? "Remove what's left" : 'Remove what it added'}
            </Button>
          )
        )}
        <Button small kind="ghost" icon="close" onClick={onDelete} ariaLabel={`Forget the trace of ${trace.name}`} />
      </div>
      <Collapse open={open}>
        <div className="border-t border-line px-4 py-2">
          {trace.installer && <p className="py-1.5 text-small text-fg-3">Installed with {trace.installer}</p>}
          {trace.alsoInstalled.length > 0 && (
            <p className="py-1.5 text-small text-fg-3">Installed along with it, kept out of the trace: {trace.alsoInstalled.map((n) => `«${n}»`).join(', ')}</p>
          )}
          <ul>
            {trace.items.map((item, i) => (
              <li key={item.id} className={cn('flex items-center gap-3 py-1.5', !trace.present[i] && 'opacity-50')}>
                <Icon name={KIND_ICON[item.kind]} size={15} className="flex-none text-fg-4" />
                <span title={item.label} className="min-w-0 flex-1 truncate font-mono text-small text-fg-path">
                  {item.label}
                </span>
                {!item.sure && <Tag>Inside an existing folder</Tag>}
                {!trace.present[i] && <Tag>Gone</Tag>}
              </li>
            ))}
          </ul>
        </div>
      </Collapse>
    </Card>
  );
}
