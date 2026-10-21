import { useState } from 'react';
import { CacheBreakdown } from '../components/cleanup/CacheBreakdown';
import { cleanupGroupsOf } from '../components/cleanup/groups';
import { PageBody, PageHeader } from '../components/layout/Page';
import { api } from '../lib/api';
import { CLEANUP_GROUPS } from '../lib/cleanupGroups';
import { cn } from '../lib/cn';
import { formatBytes, plural } from '../lib/format';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Card, Icon, ListCard, ListRow, Loading, Note, Spinner, Tag } from '../ui';

export function Cleanup() {
  const attempt = useAppStore((s) => s.attempt);
  const cleanup = useResource('cleanup');
  const [open, setOpen] = useState<Record<string, boolean>>({});

  const items = cleanup.data ?? [];
  const total = items.reduce((s, i) => s + i.sizeBytes, 0);
  const safe = items.filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const [value, unit] = formatBytes(safe).split(' ');

  return (
    <>
      <PageHeader title="Cleanup" subtitle="Caches, temporary files and logs that Windows and your apps rebuild on their own">
        <Button icon="refresh" onClick={cleanup.reload} disabled={cleanup.loading}>
          Scan again
        </Button>
        <Button kind="primary" icon="sparkle" disabled title="Arrives in v0.2">
          Clean {formatBytes(safe)}
        </Button>
      </PageHeader>
      <PageBody>
        {cleanup.error && <Callout message={cleanup.error} action="Try again" onAction={cleanup.reload} />}
        {!cleanup.data && cleanup.loading && <Loading label="Measuring caches – big ones can take a moment…" />}

        {cleanup.data && (
          <>
            <Card className="flex flex-wrap items-center gap-7 px-7 py-[26px]">
              <div className="min-w-0 flex-[1_1_280px]">
                <h2 className="text-small font-semibold uppercase tracking-[.06em] text-fg-3">Safe to clean</h2>
                <div className="mt-2 text-hero font-bold leading-none tracking-[-0.035em]">
                  {value} <span className="text-[28px] font-semibold tracking-[-0.01em] text-fg-3">{unit}</span>
                </div>
                <p className="mt-3 max-w-[420px] text-body leading-normal text-[#b4b4bb]">
                  of {formatBytes(total)} found. Developer caches and anything you might want to keep stay off by default.
                </p>
              </div>
              <CacheBreakdown items={items} inline />
            </Card>

            <Note>
              This version measures only. Cleaning arrives in v0.2 – caches will be deleted for good since apps rebuild them, and
              cookies, passwords, history and offline downloads are never touched.
            </Note>

            {cleanupGroupsOf(items).map((g) => (
              <ListCard key={g.group}>
                <div className="flex min-h-14 items-center gap-3 px-4 py-2.5">
                  <Icon name={CLEANUP_GROUPS[g.group].icon} style={{ color: CLEANUP_GROUPS[g.group].color }} />
                  <h2 className="flex-1 text-[14.5px] font-semibold">{CLEANUP_GROUPS[g.group].label}</h2>
                  {cleanup.loading && <Spinner size={14} />}
                  <span className="text-small text-fg-3">{formatBytes(g.bytes)}</span>
                </div>
                {g.items.map((i) => (
                  <div key={i.id}>
                    <ListRow
                      title={i.name}
                      subtitle={`${i.description} · ${plural(i.fileCount, 'file', 'files')}`}
                      expanded={!!open[i.id]}
                      onClick={() => setOpen((o) => ({ ...o, [i.id]: !o[i.id] }))}
                    >
                      {!i.defaultOn && <Tag>Optional</Tag>}
                      <span className="font-medium tabular-nums">{formatBytes(i.sizeBytes)}</span>
                      <Icon name="chevron" size={14} className={cn('text-fg-4 transition-transform', open[i.id] && 'rotate-90')} />
                    </ListRow>
                    {open[i.id] && (
                      <div className="flex flex-col items-start gap-0.5 px-4 pb-3">
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
                    )}
                  </div>
                ))}
              </ListCard>
            ))}
          </>
        )}
      </PageBody>
    </>
  );
}
