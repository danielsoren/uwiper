import { useEffect, useState } from 'react';
import { Icon, type IconName } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Empty, ErrorBox, Loading, Note, Pill, Segmented, Switch, type PillTone } from '../components/ui';
import { api } from '../lib/api';
import type { ServiceItem } from '../lib/types';
import { useStore } from '../state/store';

type Tab = 'startup' | 'services' | 'tasks' | 'drivers';

interface Row {
  key: string;
  name: string;
  sub: string;
  path: string | null;
  pill: string;
  tone: PillTone;
  problem: boolean;
  windows: boolean;
}

const START: Record<ServiceItem['startType'], string> = {
  boot: 'Boot',
  system: 'System',
  automatic: 'Automatic',
  manual: 'Manual',
  disabled: 'Disabled',
};

const HINT: Record<Tab, string> = {
  startup: 'Programs that start when you sign in, from the Run keys and Startup folders. Enabled/disabled is the same switch Task Manager uses.',
  services: 'Services registered on this PC. “File missing” means the service belongs to an app that is already gone.',
  tasks: 'Scheduled tasks. Tasks that run a missing file are leftovers from removed apps.',
  drivers: '',
};

const ICON: Record<Tab, IconName> = { startup: 'power', services: 'cog', tasks: 'calendar', drivers: 'cpu' };

export function System() {
  const store = useStore();
  const { startup, services, tasks, attempt } = store;
  const [tab, setTab] = useState<Tab>('startup');
  const [hideWindows, setHideWindows] = useState(true);
  const [problemsOnly, setProblemsOnly] = useState(false);

  useEffect(() => {
    startup.load();
    services.load();
    tasks.load();
  }, [startup.load, services.load, tasks.load]);

  const resource = tab === 'startup' ? startup : tab === 'services' ? services : tab === 'tasks' ? tasks : null;

  const rows: Row[] =
    tab === 'startup'
      ? (startup.data ?? []).map((i) => ({
          key: i.id,
          name: i.name,
          sub: i.location,
          path: i.target ?? i.command,
          pill: i.fileExists === false ? 'File missing' : i.enabled ? 'Enabled' : 'Disabled',
          tone: i.fileExists === false ? 'bad' : i.enabled ? 'good' : 'dim',
          problem: i.fileExists === false,
          windows: i.isWindows,
        }))
      : tab === 'services'
        ? (services.data ?? []).map((s) => ({
            key: s.name,
            name: s.displayName,
            sub: `${s.name} · ${START[s.startType]}${s.delayed ? ' (delayed)' : ''}`,
            path: s.binary,
            pill: s.fileExists === false ? 'File missing' : START[s.startType],
            tone: s.fileExists === false ? 'bad' : s.startType === 'automatic' ? 'good' : 'dim',
            problem: s.fileExists === false,
            windows: s.isWindows,
          }))
        : tab === 'tasks'
          ? (tasks.data ?? []).map((t) => ({
              key: t.name,
              name: t.name,
              sub: t.triggers,
              path: t.binary ?? t.command ?? 'COM handler',
              pill: t.fileExists === false ? 'File missing' : t.enabled ? 'Ready' : 'Disabled',
              tone: t.fileExists === false ? 'bad' : t.enabled ? 'good' : 'dim',
              problem: t.fileExists === false,
              windows: t.isWindows,
            }))
          : [];

  const visible = rows
    .filter((r) => !(hideWindows && r.windows) && (!problemsOnly || r.problem))
    .sort((a, b) => Number(b.problem) - Number(a.problem));

  const countOf = (list: { isWindows: boolean }[] | null) =>
    list ? String(list.filter((i) => !(hideWindows && i.isWindows)).length) : '…';
  const tabLabel = (label: string, n: string) => (
    <>
      {label} <span className="seg-count">{n}</span>
    </>
  );

  return (
    <>
      <PageHeader title="System" subtitle="Everything that runs without asking: startup items, services, scheduled tasks and drivers" />
      <div className="toolbar">
        <Segmented<Tab>
          label="System areas"
          value={tab}
          onChange={setTab}
          options={[
            { value: 'startup', label: tabLabel('Startup', countOf(startup.data)) },
            { value: 'services', label: tabLabel('Services', countOf(services.data)) },
            { value: 'tasks', label: tabLabel('Scheduled tasks', countOf(tasks.data)) },
            { value: 'drivers', label: 'Drivers' },
          ]}
        />
        <div className="flex-1" />
        <label className="inline-switch">
          Hide Windows items
          <Switch small checked={hideWindows} onChange={setHideWindows} label="Hide Windows items" />
        </label>
        <label className="inline-switch">
          Problems only
          <Switch small checked={problemsOnly} onChange={setProblemsOnly} label="Problems only" />
        </label>
      </div>

      <div className="page-body">
        {tab === 'drivers' ? (
          <Empty icon="cpu" title="Driver packages arrive in v0.2">
            UWiper will list the third-party driver packages in the Driver Store and show which ones no device uses
            anymore – old GPU drivers alone can take gigabytes.
          </Empty>
        ) : (
          <>
            <Note>{HINT[tab]} Read-only in this version.</Note>
            {resource?.error && (
              <ErrorBox
                message={resource.error}
                {...(resource.error.includes('administrator')
                  ? { onRetry: store.restartAsAdmin, retryLabel: 'Restart as administrator' }
                  : { onRetry: () => resource.load(true) })}
              />
            )}
            {resource?.loading && !resource.data && <Loading label="Reading…" />}
            {resource?.data && visible.length === 0 && (
              <Empty icon="check" title={problemsOnly ? 'No problems here' : 'Nothing to show'}>
                {hideWindows && !problemsOnly ? 'Windows items are hidden.' : undefined}
              </Empty>
            )}
            {visible.length > 0 && (
              <div className="list-card card">
                {visible.map((r) => (
                  <div key={r.key} className={`item-row${r.problem ? ' problem' : ''}`}>
                    <span className={`item-icon ${r.problem ? 'tone-orange' : 'tone-accent'}`}>
                      <Icon name={ICON[tab]} size={17} />
                    </span>
                    <span className="item-text">
                      <span className="item-name">{r.name}</span>
                      <span className="item-sub">{r.sub}</span>
                    </span>
                    <span className="mono item-path-col" title={r.path ?? undefined}>
                      {r.path}
                    </span>
                    <Pill tone={r.tone}>{r.pill}</Pill>
                    {r.path && !r.problem && r.path.includes('\\') && (
                      <Button
                        small
                        kind="ghost"
                        icon="folder"
                        ariaLabel={`Show ${r.name} in Explorer`}
                        onClick={() => attempt(() => api.revealPath(r.path!))}
                      />
                    )}
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </>
  );
}
