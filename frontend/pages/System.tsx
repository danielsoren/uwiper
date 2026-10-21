import { useState } from 'react';
import { PageBody, PageHeader, Toolbar } from '../components/layout/Page';
import { api } from '../lib/api';
import type { ServiceItem } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import {
  Button,
  Callout,
  Empty,
  IconTile,
  ListCard,
  ListRow,
  Note,
  Pill,
  SegmentCount,
  Segmented,
  SkeletonRows,
  Switch,
  type IconName,
  type PillTone,
} from '../ui';

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

const HINT: Record<Exclude<Tab, 'drivers'>, string> = {
  startup: 'Programs that start when you sign in, from the Run keys and Startup folders. Enabled/disabled is the same switch Task Manager uses.',
  services: 'Services registered on this PC. “File missing” means the service belongs to an app that is already gone.',
  tasks: 'Scheduled tasks. Tasks that run a missing file are leftovers from removed apps.',
};

const ICON: Record<Tab, IconName> = { startup: 'power', services: 'cog', tasks: 'calendar', drivers: 'cpu' };

export function System() {
  const attempt = useAppStore((s) => s.attempt);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const startup = useResource('startup');
  const services = useResource('services');
  const tasks = useResource('tasks');
  const [tab, setTab] = useState<Tab>('startup');
  const [hideWindows, setHideWindows] = useState(true);
  const [problemsOnly, setProblemsOnly] = useState(false);

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

  const countOf = (list: { isWindows: boolean }[] | null) => (list ? list.filter((i) => !(hideWindows && i.isWindows)).length : '…');
  const resource = tab === 'startup' ? startup : tab === 'services' ? services : tab === 'tasks' ? tasks : null;

  return (
    <>
      <PageHeader title="System" subtitle="Everything that runs without asking: startup items, services, scheduled tasks and drivers" />
      <Toolbar>
        <Segmented<Tab>
          label="System areas"
          value={tab}
          onChange={setTab}
          options={[
            { value: 'startup', label: <>Startup <SegmentCount>{countOf(startup.data)}</SegmentCount></> },
            { value: 'services', label: <>Services <SegmentCount>{countOf(services.data)}</SegmentCount></> },
            { value: 'tasks', label: <>Scheduled tasks <SegmentCount>{countOf(tasks.data)}</SegmentCount></> },
            { value: 'drivers', label: 'Drivers' },
          ]}
        />
        <div className="flex-1" />
        <label className="inline-flex items-center gap-2 text-meta text-fg-2">
          Hide Windows items
          <Switch small checked={hideWindows} onChange={setHideWindows} label="Hide Windows items" />
        </label>
        <label className="inline-flex items-center gap-2 text-meta text-fg-2">
          Problems only
          <Switch small checked={problemsOnly} onChange={setProblemsOnly} label="Problems only" />
        </label>
      </Toolbar>

      <PageBody>
        {tab === 'drivers' || !resource ? (
          <Empty icon="cpu" title="Driver packages arrive in v0.3">
            UWiper will list the third-party driver packages in the Driver Store and show which ones no device uses anymore – old
            GPU drivers alone can take gigabytes.
          </Empty>
        ) : (
          <>
            <Note>{HINT[tab as Exclude<Tab, 'drivers'>]} Entries that point to missing files can be removed on the Leftovers page.</Note>
            {resource.error && (
              <Callout
                message={resource.error}
                action={resource.error.includes('administrator') ? 'Restart as administrator' : 'Try again'}
                onAction={resource.error.includes('administrator') ? restartAsAdmin : resource.reload}
              />
            )}
            {resource.loading && !resource.data && <SkeletonRows rows={8} />}
            {resource.data && visible.length === 0 && (
              <Empty icon="check" title={problemsOnly ? 'No problems here' : 'Nothing to show'}>
                {hideWindows && !problemsOnly ? 'Windows items are hidden.' : undefined}
              </Empty>
            )}
            {visible.length > 0 && (
              <ListCard>
                {visible.map((r) => (
                  <ListRow
                    key={r.key}
                    tone={r.problem ? 'problem' : undefined}
                    leading={<IconTile icon={ICON[tab]} tone={r.problem ? 'orange' : 'accent'} small />}
                    title={r.name}
                    subtitle={r.sub}
                  >
                    <span title={r.path ?? undefined} className="min-w-0 flex-[1_1_220px] truncate font-mono text-[11.5px] text-fg-3">
                      {r.path}
                    </span>
                    <Pill tone={r.tone}>{r.pill}</Pill>
                    {r.path && !r.problem && r.path.includes('\\') && (
                      <Button small kind="ghost" icon="folder" ariaLabel={`Show ${r.name} in Explorer`} onClick={() => attempt(() => api.revealPath(r.path!))} />
                    )}
                  </ListRow>
                ))}
              </ListCard>
            )}
          </>
        )}
      </PageBody>
    </>
  );
}
