import { useEffect } from 'react';
import { Icon, type IconName } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Card, Spinner } from '../components/ui';
import { api } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { CleanupGroup } from '../lib/types';
import { brokenEntries, useStore, type Page } from '../state/store';

export const GROUP_META: Record<CleanupGroup, { label: string; color: string; icon: IconName }> = {
  windows: { label: 'Windows', color: '#0A84FF', icon: 'monitor' },
  browsers: { label: 'Browsers', color: '#64D2FF', icon: 'globe' },
  apps: { label: 'Apps', color: '#E5E5EA', icon: 'chat' },
  developer: { label: 'Developer', color: '#BF5AF2', icon: 'code' },
};

export function Overview() {
  const store = useStore();
  const { cleanup, apps, startup, services, tasks, storage, settings, env, go } = store;

  useEffect(() => {
    for (const load of [cleanup.load, apps.load, startup.load, services.load, tasks.load, storage.load]) load();
  }, [cleanup.load, apps.load, startup.load, services.load, tasks.load, storage.load]);
  const rescan = () => {
    for (const load of [cleanup.load, apps.load, startup.load, services.load, tasks.load, storage.load]) load(true);
  };

  const items = cleanup.data ?? [];
  const safe = items.filter((i) => i.defaultOn);
  const safeBytes = safe.reduce((s, i) => s + i.sizeBytes, 0);
  const devBytes = items.filter((i) => !i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const byGroup = (['windows', 'browsers', 'apps'] as CleanupGroup[])
    .map((g) => ({ group: g, bytes: safe.filter((i) => i.group === g).reduce((s, i) => s + i.sizeBytes, 0) }))
    .filter((g) => g.bytes > 0);

  const broken = brokenEntries(store);
  const enabledStartup = (startup.data ?? []).filter((i) => i.enabled).length;
  const reportedBytes = (apps.data ?? []).reduce((s, a) => s + (a.sizeBytes ?? 0), 0);
  const outsideDirs = (storage.data?.dirs ?? []).filter((d) => !d.inside).length;
  const outsideEntries = outsideDirs + (settings?.footprint.startWithWindows ? 1 : 0);

  const stats: { icon: IconName; tone: string; value: string; label: string; sub: string; page: Page; loading: boolean }[] = [
    {
      icon: 'apps',
      tone: 'accent',
      value: apps.data ? String(apps.data.length) : '–',
      label: 'Installed apps',
      sub: apps.error ?? `${formatBytes(reportedBytes)} reported by installers`,
      page: 'apps',
      loading: apps.loading,
    },
    {
      icon: 'traces',
      tone: 'purple',
      value: startup.data || services.data || tasks.data ? String(broken.total) : '–',
      label: 'Entries pointing nowhere',
      sub: 'Startup, services and tasks whose files are gone',
      page: 'leftovers',
      loading: startup.loading || services.loading || tasks.loading,
    },
    {
      icon: 'power',
      tone: 'orange',
      value: startup.data ? String(enabledStartup) : '–',
      label: 'Start with Windows',
      sub: startup.error ?? `${plural(startup.data?.length ?? 0, 'entry', 'entries')} in total`,
      page: 'system',
      loading: startup.loading,
    },
    {
      icon: 'archive',
      tone: 'green',
      value: 'Empty',
      label: 'Quarantine',
      sub: 'Nothing has been removed yet',
      page: 'quarantine',
      loading: false,
    },
  ];

  return (
    <>
      <PageHeader title="Overview" subtitle="A read-only look at your PC – this version shows, it doesn't remove anything yet">
        <Button icon="refresh" onClick={rescan}>
          Scan again
        </Button>
      </PageHeader>
      <div className="page-body">
        <Card className="hero">
          <div className="hero-main">
            <h2 className="eyebrow">Caches you could free</h2>
            {cleanup.loading && !cleanup.data ? (
              <div className="hero-loading">
                <Spinner size={26} />
                <span>Measuring caches…</span>
              </div>
            ) : (
              <div className="hero-value">
                {formatBytes(safeBytes).split(' ')[0]} <span>{formatBytes(safeBytes).split(' ')[1]}</span>
              </div>
            )}
            <p className="hero-text">
              Temporary files and caches that Windows and your apps rebuild on their own.
              {devBytes > 0 && ` Plus ${formatBytes(devBytes)} of developer and app caches that stay off unless you pick them.`}
            </p>
            <div className="row-gap">
              <Button kind="primary" icon="sparkle" onClick={() => go('cleanup')}>
                See what's inside
              </Button>
            </div>
          </div>
          <div className="hero-side">
            <div className="stack-bar" role="img" aria-label={byGroup.map((g) => `${GROUP_META[g.group].label} ${formatBytes(g.bytes)}`).join(', ')}>
              {byGroup.map((g) => (
                <span key={g.group} style={{ flexGrow: g.bytes, background: GROUP_META[g.group].color }} />
              ))}
              {byGroup.length === 0 && <span className="stack-bar-empty" />}
            </div>
            <div className="legend">
              {byGroup.map((g) => (
                <button type="button" key={g.group} className="legend-row" onClick={() => go('cleanup')}>
                  <span className="legend-dot" style={{ background: GROUP_META[g.group].color }} />
                  <span className="legend-label">{GROUP_META[g.group].label}</span>
                  <span className="legend-value">{formatBytes(g.bytes)}</span>
                  <Icon name="chevron" size={14} className="muted" />
                </button>
              ))}
            </div>
          </div>
        </Card>

        <div className="stat-grid">
          {stats.map((s) => (
            <button type="button" key={s.label} className="stat" onClick={() => go(s.page)}>
              <span className={`stat-icon tone-${s.tone}`}>
                <Icon name={s.icon} />
              </span>
              <span className="stat-value">{s.loading && s.value === '–' ? <Spinner size={20} /> : s.value}</span>
              <span className="stat-label">{s.label}</span>
              <span className="stat-sub">{s.sub}</span>
            </button>
          ))}
        </div>

        <div className="two-col">
          <Card className="pad">
            <div className="card-head">
              <span className="stat-icon tone-accent">
                <Icon name="folder" />
              </span>
              <h2>Where UWiper lives</h2>
            </div>
            <p className="mono path-line">{env?.home ?? '…'}</p>
            <p className="muted-text">
              {outsideEntries === 0
                ? 'Everything UWiper keeps is in this one folder. Nothing in AppData, ProgramData or the registry.'
                : `${plural(outsideEntries, 'thing', 'things')} outside this folder, all listed in Settings.`}
            </p>
            <div className="row-gap">
              <Button small icon="folder" onClick={() => env && store.attempt(() => api.openFolder(env.home))}>
                Open folder
              </Button>
              <Button small kind="ghost" onClick={() => go('settings')}>
                Storage settings
              </Button>
            </div>
          </Card>
          <Card className="pad">
            <div className="card-head">
              <span className="stat-icon tone-green">
                <Icon name="shield" />
              </span>
              <h2>Safe by design</h2>
            </div>
            <ul className="check-list">
              <li>
                <Icon name="check" size={16} stroke={2.2} />
                This version only reads – it never changes your system
              </li>
              <li>
                <Icon name="check" size={16} stroke={2.2} />
                Removals will go to quarantine first, restorable for {settings?.safety.keepDays ?? 30} days
              </li>
              <li>
                <Icon name="check" size={16} stroke={2.2} />
                Windows components are never offered for removal
              </li>
            </ul>
          </Card>
        </div>
      </div>
    </>
  );
}
