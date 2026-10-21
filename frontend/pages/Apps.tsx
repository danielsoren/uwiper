import { useEffect, useMemo, useState } from 'react';
import { AppDetail, type Measure } from '../components/apps/AppDetail';
import { AppRow } from '../components/apps/AppRow';
import { PageBody, PageHeader, Toolbar } from '../components/layout/Page';
import { api } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { InstalledApp } from '../lib/types';
import { useResource } from '../store/hooks';
import { Button, Callout, Empty, Field, Loading, SegmentCount, Segmented } from '../ui';

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
  const [measured, setMeasured] = useState<Record<string, Measure>>({});

  const all = useMemo(() => apps.data ?? [], [apps.data]);
  const sizeOf = (a: InstalledApp) => {
    const m = measured[a.id];
    return typeof m === 'number' ? m : a.sizeBytes;
  };

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const size = (a: InstalledApp) => (typeof measured[a.id] === 'number' ? (measured[a.id] as number) : a.sizeBytes ?? 0);
    return all
      .filter((a) => (source === 'all' || a.source === source) && (!q || `${a.name} ${a.publisher ?? ''}`.toLowerCase().includes(q)))
      .sort((x, y) => {
        if (sort === 'size') return size(y) - size(x);
        if (sort === 'date') return (y.installDate ?? '').localeCompare(x.installDate ?? '');
        return x.name.localeCompare(y.name, undefined, { sensitivity: 'base' });
      });
  }, [all, query, source, sort, measured]);

  const selected = rows.find((a) => a.id === selectedId) ?? rows[0] ?? null;

  // Measure the selected app's folder once.
  useEffect(() => {
    if (!selected?.installLocation || measured[selected.id] !== undefined) return;
    const id = selected.id;
    setMeasured((m) => ({ ...m, [id]: 'loading' }));
    api.folderSize(selected.installLocation).then(
      (bytes) => setMeasured((m) => ({ ...m, [id]: bytes })),
      () => setMeasured((m) => ({ ...m, [id]: 'error' })),
    );
  }, [selected, measured]);

  const count = (s: Source) => all.filter((a) => s === 'all' || a.source === s).length;
  const reported = all.reduce((s, a) => s + (a.sizeBytes ?? 0), 0);

  return (
    <>
      <PageHeader
        title="Apps"
        subtitle={apps.data ? `${plural(all.length, 'app', 'apps')} · ${formatBytes(reported)} reported by installers` : 'Reading the list of installed apps…'}
      >
        <Field icon="search" label="Search apps" placeholder="Name or publisher" value={query} onChange={setQuery} className="w-[260px]" />
        <Button icon="refresh" ariaLabel="Reload the list" onClick={apps.reload} />
      </PageHeader>

      <Toolbar>
        <Segmented<Source>
          label="Install source"
          value={source}
          onChange={setSource}
          options={[
            { value: 'all', label: <>All <SegmentCount>{count('all')}</SegmentCount></> },
            { value: 'win32', label: <>Classic <SegmentCount>{count('win32')}</SegmentCount></> },
            { value: 'msi', label: <>Windows Installer <SegmentCount>{count('msi')}</SegmentCount></> },
          ]}
        />
        <div className="flex-1" />
        <Button kind="ghost" small icon="sort" onClick={() => setSort(NEXT_SORT[sort])}>
          {SORT_LABEL[sort]}
        </Button>
      </Toolbar>

      {!apps.data ? (
        <PageBody>
          {apps.error ? <Callout message={apps.error} action="Try again" onAction={apps.reload} /> : <Loading label="Reading installed apps from the registry…" />}
        </PageBody>
      ) : (
        <div className="flex min-h-0 flex-1 border-t border-line">
          <div role="listbox" aria-label="Installed apps" className="scroll-thin min-w-0 flex-[3_1_380px] overflow-auto border-r border-line pb-3 pl-4 pr-2.5 pt-2">
            {rows.map((a) => (
              <AppRow key={a.id} app={a} selected={selected?.id === a.id} sizeBytes={sizeOf(a)} onSelect={() => setSelectedId(a.id)} />
            ))}
            {rows.length === 0 && <div className="py-12 text-center text-fg-3">No apps match</div>}
          </div>
          <div className="scroll-thin min-w-0 flex-[2_1_360px] overflow-auto">
            {selected ? <AppDetail app={selected} measure={measured[selected.id]} /> : <Empty icon="apps" title="No app selected" />}
          </div>
        </div>
      )}
    </>
  );
}
