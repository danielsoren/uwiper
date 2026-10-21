import { useState } from 'react';
import { CacheBreakdown } from '../components/cleanup/CacheBreakdown';
import { CleanDialog } from '../components/cleanup/CleanDialog';
import { cleanupGroupsOf } from '../components/cleanup/groups';
import { PageBody, PageHeader } from '../components/layout/Page';
import { api } from '../lib/api';
import { CLEANUP_GROUPS } from '../lib/cleanupGroups';
import { cn } from '../lib/cn';
import { formatBytes, plural } from '../lib/format';
import type { CleanupItem } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Card, Checkbox, Collapse, Icon, ListCard, Note, Skeleton, SkeletonRows, Tag } from '../ui';

export function Cleanup() {
  const attempt = useAppStore((s) => s.attempt);
  const cleanup = useResource('cleanup');
  const [open, setOpen] = useState<Record<string, boolean>>({});
  // What the user ticked or unticked; everything else follows its default.
  const [picks, setPicks] = useState<Record<string, boolean>>({});
  const [cleaning, setCleaning] = useState<CleanupItem[] | null>(null);

  const items = cleanup.data ?? [];
  const isPicked = (i: CleanupItem) => i.sizeBytes > 0 && (picks[i.id] ?? i.defaultOn);
  const picked = items.filter(isPicked);
  const pickedBytes = picked.reduce((s, i) => s + i.sizeBytes, 0);
  const total = items.reduce((s, i) => s + i.sizeBytes, 0);
  const safe = items.filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const [value, unit] = formatBytes(safe).split(' ');
  const pick = (list: CleanupItem[], on: boolean) => setPicks((p) => ({ ...p, ...Object.fromEntries(list.map((i) => [i.id, on])) }));

  return (
    <>
      <PageHeader title="Cleanup" subtitle="Caches, temporary files and logs that Windows and your apps rebuild on their own">
        <Button icon="refresh" onClick={cleanup.reload} busy={cleanup.loading}>
          Scan again
        </Button>
        <Button kind="primary" icon="sparkle" disabled={!cleanup.data || cleanup.loading || pickedBytes === 0} onClick={() => setCleaning(picked)}>
          {cleanup.data && pickedBytes > 0 ? `Clean ${formatBytes(pickedBytes)}` : 'Clean'}
        </Button>
      </PageHeader>
      <PageBody>
        {cleanup.error && <Callout message={cleanup.error} action="Try again" onAction={cleanup.reload} />}
        {!cleanup.data && cleanup.loading && (
          <>
            <Card className="flex flex-col gap-3 px-7 py-[26px]">
              <Skeleton className="w-24" />
              <Skeleton className="h-14 w-52 rounded-xl" />
              <Skeleton className="w-80 max-w-full" />
            </Card>
            <SkeletonRows rows={4} tile={false} />
            <SkeletonRows rows={3} tile={false} />
          </>
        )}

        {cleanup.data && (
          <>
            <Card className="flex flex-wrap items-center gap-7 px-7 py-[26px]">
              <div className="min-w-0 flex-[1_1_280px]">
                <h2 className="text-small font-semibold uppercase tracking-[.06em] text-fg-3">Safe to clean</h2>
                <div className="mt-2 text-hero font-bold leading-none tracking-[-0.035em]">
                  {safe > 0 ? (
                    <>
                      {value} <span className="text-[28px] font-semibold tracking-[-0.01em] text-fg-3">{unit}</span>
                    </>
                  ) : (
                    'All clean'
                  )}
                </div>
                <p className="mt-3 max-w-[420px] text-body leading-normal text-[#b4b4bb]">
                  of {formatBytes(total)} found. Developer caches and anything you might want to keep stay off by default.
                </p>
              </div>
              <CacheBreakdown items={items} inline />
            </Card>

            <Note>
              Caches are deleted for good – apps rebuild them when needed. Cookies, passwords, history and offline downloads are
              never touched. Files in use are skipped, so close your browsers first to clean their caches completely.
            </Note>

            {cleanupGroupsOf(items).map((g) => {
              const nonEmpty = g.items.filter((i) => i.sizeBytes > 0);
              const count = nonEmpty.filter(isPicked).length;
              return (
                <ListCard key={g.group}>
                  <div className="flex min-h-14 items-center gap-3 px-4 py-2.5">
                    <Checkbox
                      label={`Select all: ${CLEANUP_GROUPS[g.group].label}`}
                      checked={nonEmpty.length > 0 && count === nonEmpty.length}
                      mixed={count > 0 && count < nonEmpty.length}
                      disabled={nonEmpty.length === 0}
                      onChange={(on) => pick(nonEmpty, on)}
                    />
                    <Icon name={CLEANUP_GROUPS[g.group].icon} style={{ color: CLEANUP_GROUPS[g.group].color }} />
                    <h2 className="flex-1 text-[14.5px] font-semibold">{CLEANUP_GROUPS[g.group].label}</h2>
                    <span className="text-small text-fg-3">{formatBytes(g.bytes)}</span>
                  </div>
                  {g.items.map((i) => (
                    <div key={i.id}>
                      <div className="flex min-h-[60px] items-center gap-3 pl-4 hover:bg-hover">
                        <Checkbox label={`Clean ${i.name}`} checked={isPicked(i)} disabled={i.sizeBytes === 0} onChange={(on) => pick([i], on)} />
                        <button
                          type="button"
                          aria-expanded={!!open[i.id]}
                          onClick={() => setOpen((o) => ({ ...o, [i.id]: !o[i.id] }))}
                          className="flex min-w-0 flex-1 items-center gap-3.5 self-stretch py-2.5 pr-4 text-left"
                        >
                          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span className="truncate text-label font-semibold">{i.name}</span>
                            <span className="text-small text-fg-3">{`${i.description} · ${plural(i.fileCount, 'file', 'files')}`}</span>
                          </span>
                          {i.detected && <Tag>Found by name</Tag>}
                          {!i.defaultOn && <Tag>Optional</Tag>}
                          <span className="font-medium tabular-nums">{formatBytes(i.sizeBytes)}</span>
                          <Icon
                            name="chevron"
                            size={14}
                            className={cn('text-fg-4 transition-transform', open[i.id] && 'rotate-90', i.paths.length === 0 && 'invisible')}
                          />
                        </button>
                      </div>
                      <Collapse open={!!open[i.id]}>
                        <div className="flex flex-col items-start gap-0.5 pb-3 pl-[46px] pr-4 pt-1">
                          {i.paths.map((p) => (
                            <button
                              key={p}
                              type="button"
                              onClick={() => attempt(() => api.revealPath(p))}
                              className="break-all rounded-md px-1.5 py-[3px] text-left font-mono text-small text-fg-3 hover:bg-hover hover:text-accent-soft"
                            >
                              {p}
                            </button>
                          ))}
                        </div>
                      </Collapse>
                    </div>
                  ))}
                </ListCard>
              );
            })}
          </>
        )}
      </PageBody>

      {cleaning && (
        <CleanDialog
          items={cleaning}
          onClose={(cleaned) => {
            setCleaning(null);
            if (cleaned) {
              setPicks({});
              cleanup.reload();
            }
          }}
        />
      )}
    </>
  );
}
