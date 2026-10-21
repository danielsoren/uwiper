import { useEffect, useMemo, useState } from 'react';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Empty, ErrorBox, Loading, Note, Segmented, Spinner } from '../components/ui';
import { api, errorText } from '../lib/api';
import { formatBytes, formatDate, initial, plural, tileColor } from '../lib/format';
import type { InstalledApp } from '../lib/types';
import { useStore } from '../state/store';

type Source = 'all' | 'win32' | 'msi';
type Sort = 'name' | 'size' | 'date';

const SORT_LABEL: Record<Sort, string> = { name: 'By name', size: 'By size', date: 'By install date' };
const NEXT_SORT: Record<Sort, Sort> = { name: 'size', size: 'date', date: 'name' };

type Measure = number | 'loading' | 'error';

export function Apps() {
  const { apps } = useStore();
  const [query, setQuery] = useState('');
  const [source, setSource] = useState<Source>('all');
  const [sort, setSort] = useState<Sort>('name');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [measured, setMeasured] = useState<Record<string, Measure>>({});

  useEffect(() => apps.load(), [apps.load]);

  const all = apps.data ?? [];
  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const list = all.filter(
      (a) =>
        (source === 'all' || a.source === source) &&
        (!q || `${a.name} ${a.publisher ?? ''}`.toLowerCase().includes(q)),
    );
    const sizeOf = (a: InstalledApp) => (typeof measured[a.id] === 'number' ? (measured[a.id] as number) : a.sizeBytes ?? 0);
    return [...list].sort((x, y) => {
      if (sort === 'size') return sizeOf(y) - sizeOf(x);
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

  const counts = {
    all: all.length,
    win32: all.filter((a) => a.source === 'win32').length,
    msi: all.filter((a) => a.source === 'msi').length,
  };
  const reported = all.reduce((s, a) => s + (a.sizeBytes ?? 0), 0);

  return (
    <>
      <PageHeader
        title="Apps"
        subtitle={apps.data ? `${plural(all.length, 'app', 'apps')} · ${formatBytes(reported)} reported by installers` : 'Reading the list of installed apps…'}
      >
        <label className="search-field">
          <Icon name="search" size={15} stroke={2} />
          <input
            type="search"
            placeholder="Name or publisher"
            aria-label="Search apps"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <Button icon="refresh" ariaLabel="Reload the list" onClick={() => apps.load(true)} />
      </PageHeader>

      <div className="toolbar">
        <Segmented<Source>
          label="Install source"
          value={source}
          onChange={setSource}
          options={[
            { value: 'all', label: <>All <span className="seg-count">{counts.all}</span></> },
            { value: 'win32', label: <>Classic <span className="seg-count">{counts.win32}</span></> },
            { value: 'msi', label: <>Windows Installer <span className="seg-count">{counts.msi}</span></> },
          ]}
        />
        <div className="flex-1" />
        <Button kind="ghost" small icon="sort" onClick={() => setSort(NEXT_SORT[sort])}>
          {SORT_LABEL[sort]}
        </Button>
      </div>

      {apps.error && !apps.data ? (
        <div className="page-body">
          <ErrorBox message={apps.error} onRetry={() => apps.load(true)} />
        </div>
      ) : !apps.data ? (
        <div className="page-body">
          <Loading label="Reading installed apps from the registry…" />
        </div>
      ) : (
        <div className="split">
          <div className="split-list scroll" role="listbox" aria-label="Installed apps">
            {rows.map((a) => {
              const on = selected?.id === a.id;
              const m = measured[a.id];
              return (
                <button
                  key={a.id}
                  type="button"
                  role="option"
                  aria-selected={on}
                  className={`app-row${on ? ' selected' : ''}`}
                  onClick={() => setSelectedId(a.id)}
                >
                  <span className="app-tile" style={{ background: tileColor(a.name) }} aria-hidden="true">
                    {initial(a.name)}
                  </span>
                  <span className="app-text">
                    <span className="app-name">{a.name}</span>
                    <span className="app-sub">{a.publisher ?? 'Unknown publisher'}</span>
                  </span>
                  {a.source === 'msi' && <span className="app-badge">MSI</span>}
                  {a.scope === 'user' && <span className="app-badge">User</span>}
                  <span className="app-size">{formatBytes(typeof m === 'number' ? m : a.sizeBytes)}</span>
                </button>
              );
            })}
            {rows.length === 0 && <div className="list-empty">No apps match</div>}
          </div>
          <div className="split-detail scroll">
            {selected ? <AppDetail app={selected} measure={measured[selected.id]} /> : <Empty icon="apps" title="No app selected" />}
          </div>
        </div>
      )}
    </>
  );
}

function AppDetail({ app, measure }: { app: InstalledApp; measure: Measure | undefined }) {
  const { attempt, notify } = useStore();
  const [copied, setCopied] = useState(false);
  const command = app.quietUninstallString ?? app.uninstallString;

  const copy = () => {
    if (!command) return;
    navigator.clipboard.writeText(command).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      (e) => notify(errorText(e)),
    );
  };

  return (
    <div className="detail">
      <div className="detail-head">
        <span className="app-tile large" style={{ background: tileColor(app.name) }} aria-hidden="true">
          {initial(app.name)}
        </span>
        <div className="min-0">
          <h2 className="detail-title">{app.name}</h2>
          <div className="detail-sub">
            {app.publisher ?? 'Unknown publisher'}
            {app.version && ` · version ${app.version}`}
          </div>
        </div>
      </div>

      <dl className="facts">
        <div>
          <dt>Size on disk</dt>
          <dd>
            {measure === 'loading' ? (
              <span className="inline-loading">
                <Spinner size={13} /> Measuring…
              </span>
            ) : typeof measure === 'number' ? (
              formatBytes(measure)
            ) : (
              <>
                {formatBytes(app.sizeBytes)} <span className="muted-text">reported</span>
              </>
            )}
          </dd>
        </div>
        <div>
          <dt>Installed</dt>
          <dd>{formatDate(app.installDate)}</dd>
        </div>
        <div>
          <dt>Installer</dt>
          <dd>{app.source === 'msi' ? 'Windows Installer (MSI)' : 'Classic installer'}</dd>
        </div>
        <div>
          <dt>Installed for</dt>
          <dd>
            {app.scope === 'user' ? 'Current user' : 'All users'}
            {app.arch && ` · ${app.arch}`}
          </dd>
        </div>
        <div className="span-2">
          <dt>Location</dt>
          <dd className="mono wrap">{app.installLocation ?? 'Not reported by the installer'}</dd>
        </div>
        <div className="span-2">
          <dt>Registry entry</dt>
          <dd className="mono wrap">{app.registryKey}</dd>
        </div>
        {command && (
          <div className="span-2">
            <dt>Uninstall command</dt>
            <dd className="mono wrap">{command}</dd>
          </div>
        )}
      </dl>

      <div className="row-gap">
        <Button kind="danger" icon="trash" disabled title="Arrives in v0.2">
          Uninstall completely
        </Button>
        {app.installLocation && (
          <Button icon="folder" onClick={() => attempt(() => api.openFolder(app.installLocation!))}>
            Open folder
          </Button>
        )}
        {command && (
          <Button kind="ghost" onClick={copy}>
            {copied ? 'Copied' : 'Copy uninstall command'}
          </Button>
        )}
      </div>

      <Note>
        <strong>Coming in v0.2:</strong> one-click uninstall that runs the app's own uninstaller, then finds
        everything it left behind – files, registry keys, services, tasks, shortcuts – and moves them to quarantine.
      </Note>
    </div>
  );
}
