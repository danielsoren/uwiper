import { useEffect, useMemo, useState } from 'react';
import { AppDetail } from '../components/apps/AppDetail';
import { AppRow } from '../components/apps/AppRow';
import { AppsSkeleton } from '../components/apps/AppsSkeleton';
import { PageBody, PageHeader, Toolbar } from '../components/layout/Page';
import { formatBytes, plural } from '../lib/format';
import { useFocus, useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Empty, Field, SegmentCount, Segmented } from '../ui';

type Source = 'all' | 'win32' | 'msi';
type Sort = 'name' | 'size' | 'date';

const SORT_LABEL: Record<Sort, string> = { name: 'By name', size: 'By size', date: 'By install date' };
const NEXT_SORT: Record<Sort, Sort> = { name: 'size', size: 'date', date: 'name' };

export function Apps() {
  const apps = useResource('apps');
  const [query, setQuery] = useState('');
  const [source, setSource] = useState<Source>('all');
  const [sort, setSort] = useState<Sort>('name');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const traces = useAppStore((s) => s.res.traces.data);
  const tracked = useMemo(() => new Set((traces ?? []).map((t) => t.appId).filter(Boolean)), [traces]);

  // Installs' "Uninstall…" opens the app it was tracked for.
  const focus = useFocus(!!apps.data);
  useEffect(() => {
    if (focus) setSelectedId(focus);
  }, [focus]);

  const all = useMemo(() => apps.data ?? [], [apps.data]);
  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    return all
      .filter((a) => (source === 'all' || a.source === source) && (!q || `${a.name} ${a.publisher ?? ''}`.toLowerCase().includes(q)))
      .sort((x, y) => {
        if (sort === 'size') return (y.sizeBytes ?? 0) - (x.sizeBytes ?? 0);
        if (sort === 'date') return (y.installDate ?? '').localeCompare(x.installDate ?? '');
        return x.name.localeCompare(y.name, undefined, { sensitivity: 'base' });
      });
  }, [all, query, source, sort]);

  const selected = rows.find((a) => a.id === selectedId) ?? rows[0] ?? null;
  const count = (s: Source) => apps.data && <SegmentCount>{all.filter((a) => s === 'all' || a.source === s).length}</SegmentCount>;
  const total = all.reduce((s, a) => s + (a.sizeBytes ?? 0), 0);

  return (
    <>
      <PageHeader
        title="Apps"
        subtitle={apps.data ? `${plural(all.length, 'app', 'apps')} · ${formatBytes(total)} on disk` : 'Measuring installed apps…'}
      >
        <Field icon="search" label="Search apps" placeholder="Name or publisher" value={query} onChange={setQuery} className="w-[260px]" />
        <Button icon="refresh" ariaLabel="Reload the list" onClick={apps.reload} busy={apps.loading} />
      </PageHeader>

      <Toolbar>
        <Segmented<Source>
          label="Install source"
          value={source}
          onChange={setSource}
          options={[
            { value: 'all', label: <>All {count('all')}</> },
            { value: 'win32', label: <>Classic {count('win32')}</> },
            { value: 'msi', label: <>Windows Installer {count('msi')}</> },
          ]}
        />
        <div className="flex-1" />
        <Button kind="ghost" small icon="sort" onClick={() => setSort(NEXT_SORT[sort])}>
          {SORT_LABEL[sort]}
        </Button>
      </Toolbar>

      {apps.error && !apps.data ? (
        <PageBody>
          <Callout message={apps.error} action="Try again" onAction={apps.reload} />
        </PageBody>
      ) : !apps.data ? (
        <AppsSkeleton />
      ) : (
        <div className="flex min-h-0 flex-1 border-t border-line">
          <div role="listbox" aria-label="Installed apps" className="scroll-thin min-w-0 flex-[3_1_380px] overflow-auto border-r border-line pb-3 pl-4 pr-2.5 pt-2">
            {rows.map((a) => (
              <AppRow key={a.id} app={a} tracked={tracked.has(a.id)} selected={selected?.id === a.id} onSelect={() => setSelectedId(a.id)} />
            ))}
            {rows.length === 0 && <div className="py-12 text-center text-fg-3">No apps match</div>}
          </div>
          <div className="scroll-thin min-w-0 flex-[2_1_360px] overflow-auto">
            {selected ? <AppDetail app={selected} /> : <Empty icon="apps" title="No app selected" />}
          </div>
        </div>
      )}
    </>
  );
}
