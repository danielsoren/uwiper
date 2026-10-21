import { useState } from 'react';
import { PageBody, PageHeader } from '../components/layout/Page';
import { ProblemList } from '../components/leftovers/ProblemList';
import { api, errorText } from '../lib/api';
import { cn } from '../lib/cn';
import { daysAgo, formatBytes, plural } from '../lib/format';
import type { Batch, Problem, Target } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Card, Dialog, Empty, Icon, IconTile, SkeletonRows, type IconName } from '../ui';

const TARGET_ICON: Record<Target['kind'], IconName> = {
  path: 'folder',
  registryKey: 'cube',
  registryValue: 'power',
  service: 'cog',
  task: 'calendar',
};

function BatchCard({
  batch,
  keepDays,
  busy,
  onRestore,
  onDelete,
}: {
  batch: Batch;
  keepDays: number;
  busy: boolean;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const [open, setOpen] = useState(false);
  const bytes = batch.items.reduce((s, i) => s + i.sizeBytes, 0);
  const left = Math.max(0, keepDays - Math.floor((Date.now() / 1000 - batch.createdAt) / 86_400));
  return (
    <Card className="overflow-hidden">
      <div className="flex flex-wrap items-center gap-3.5 px-4 py-3.5">
        <IconTile icon="archive" tone="green" />
        <button type="button" aria-expanded={open} onClick={() => setOpen(!open)} className="flex min-w-0 flex-1 items-center gap-2.5 text-left">
          <span className="min-w-0 flex-1">
            <span className="block truncate text-label font-semibold">{batch.title}</span>
            <span className="mt-0.5 block text-small text-fg-3">
              {daysAgo(batch.createdAt)} · {plural(batch.items.length, 'item', 'items')}
              {bytes > 0 && ` · ${formatBytes(bytes)}`} · {left === 0 ? 'deleted for good today' : `deleted for good in ${plural(left, 'day', 'days')}`}
            </span>
          </span>
          <Icon name="chevron" size={14} className={cn('flex-none text-fg-4 transition-transform', open && 'rotate-90')} />
        </button>
        <Button small icon="refresh" busy={busy} onClick={onRestore}>
          Restore
        </Button>
        <Button small kind="ghost" icon="trash" disabled={busy} onClick={onDelete} ariaLabel={`Delete ${batch.title} for good`} />
      </div>
      {open && (
        <ul className="border-t border-line px-4 py-2">
          {batch.items.map((item, i) => (
            <li key={`${item.label}-${i}`} className="flex items-center gap-3 py-1.5">
              <Icon name={TARGET_ICON[item.target.kind]} size={15} className="flex-none text-fg-4" />
              <span className="min-w-0 flex-1">
                <span title={item.label} className="block truncate font-mono text-small text-fg-path">
                  {item.label}
                </span>
                {item.note && <span className="block text-small text-warn-fg">{item.note}</span>}
              </span>
              {item.sizeBytes > 0 && <span className="text-small tabular-nums text-fg-3">{formatBytes(item.sizeBytes)}</span>}
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}

export function Quarantine() {
  const go = useAppStore((s) => s.go);
  const attempt = useAppStore((s) => s.attempt);
  const notify = useAppStore((s) => s.notify);
  const refresh = useAppStore((s) => s.refresh);
  const updateSettings = useAppStore((s) => s.updateSettings);
  const safety = useAppStore((s) => s.settings?.safety);
  const keepDays = safety?.keepDays ?? 30;
  const storage = useResource('storage');
  const batches = useResource('quarantine');
  const dir = storage.data?.dirs.find((d) => d.key === 'quarantine');
  const [busy, setBusy] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<Batch | null>(null);
  const [problems, setProblems] = useState<Problem[]>([]);

  const list = batches.data ?? [];
  const total = list.reduce((s, b) => s + b.items.reduce((t, i) => t + i.sizeBytes, 0), 0);

  const restore = async (batch: Batch) => {
    setBusy(batch.id);
    setProblems([]);
    try {
      const report = await api.restoreBatch(batch.id);
      setProblems(report.problems);
      if (report.restored > 0) notify(`${plural(report.restored, 'item', 'items')} put back`);
      if (report.restartNeeded) notify('Restart Windows to bring back the restored services');
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(null);
    refresh('quarantine', 'storage', 'apps', 'startup', 'services', 'tasks');
  };

  const remove = async (batch: Batch) => {
    setDeleting(null);
    setBusy(batch.id);
    try {
      await api.deleteBatch(batch.id);
      notify(`${batch.title} deleted for good`);
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(null);
    refresh('quarantine', 'storage');
  };

  return (
    <>
      <PageHeader
        title="Quarantine"
        subtitle={
          batches.data && list.length
            ? `${plural(list.length, 'batch', 'batches')} · ${formatBytes(total)} · each one is deleted for good after ${keepDays} days`
            : `Everything UWiper removes waits here for ${keepDays} days, ready to put back`
        }
      />
      <PageBody>
        {safety && !safety.quarantine && (
          <Callout
            message="Quarantine is off – UWiper deletes what you remove for good."
            action="Turn it on"
            onAction={() => updateSettings((s) => ({ ...s, safety: { ...s.safety, quarantine: true } }))}
          />
        )}
        <ProblemList title={`${plural(problems.length, "item wasn't", "items weren't")} restored`} problems={problems} />

        {batches.loading && !batches.data && <SkeletonRows rows={3} />}
        {batches.error && <Callout message={batches.error} action="Try again" onAction={batches.reload} />}
        {batches.data && list.length === 0 && (
          <Empty icon="archive" title="Quarantine is empty">
            When you remove leftovers, they land here first – files together with .reg backups of registry keys, services and
            scheduled tasks – so any of it can be put back in one click.
          </Empty>
        )}
        {list.map((b) => (
          <BatchCard key={b.id} batch={b} keepDays={keepDays} busy={busy === b.id} onRestore={() => restore(b)} onDelete={() => setDeleting(b)} />
        ))}

        <Card padded className="flex flex-wrap items-center gap-4">
          <IconTile icon="folder" />
          <div className="min-w-0 flex-1">
            <div className="text-label font-semibold">Quarantine folder</div>
            <div className="mt-1 select-text break-all font-mono text-small text-fg-path">{dir?.path ?? '…'}</div>
            {dir && !dir.inside && <div className="text-small text-fg-3">Outside the UWiper folder</div>}
          </div>
          <Button
            small
            disabled={!dir?.exists}
            title={dir?.exists ? undefined : 'Created when something is first quarantined'}
            onClick={() => dir && attempt(() => api.openFolder(dir.path))}
          >
            Open
          </Button>
          <Button small kind="ghost" onClick={() => go('settings')}>
            Change
          </Button>
        </Card>
      </PageBody>

      {deleting && (
        <Dialog
          label="Delete for good"
          onClose={() => setDeleting(null)}
          footer={
            <>
              <Button onClick={() => setDeleting(null)}>Cancel</Button>
              <Button kind="danger" icon="trash" onClick={() => remove(deleting)}>
                Delete for good
              </Button>
            </>
          }
        >
          <h2 className="text-h2 font-bold">Delete “{deleting.title}” for good?</h2>
          <p className="mt-2 text-body leading-normal text-fg-3">
            {plural(deleting.items.length, 'item', 'items')} can't be put back afterwards.
          </p>
        </Dialog>
      )}
    </>
  );
}
