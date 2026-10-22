import { useEffect, useState, type ReactNode } from 'react';
import { PageBody, PageHeader, Toolbar } from '../components/layout/Page';
import { ProblemList } from '../components/leftovers/ProblemList';
import { DriverList } from '../components/system/DriverList';
import { api, errorText } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { DriverItem, Problem, ServiceItem, ServiceStart } from '../lib/types';
import { useFocus, useResource } from '../store/hooks';
import { useAppStore, type ResourceKey } from '../store/useAppStore';
import {
  Button,
  Callout,
  Dialog,
  Empty,
  IconTile,
  ListCard,
  ListRow,
  Note,
  Pill,
  SegmentCount,
  Segmented,
  Select,
  SkeletonRows,
  Switch,
  type IconName,
  type PillTone,
} from '../ui';

type Tab = 'startup' | 'services' | 'tasks' | 'drivers';
const TABS: Tab[] = ['startup', 'services', 'tasks', 'drivers'];

interface Row {
  key: string;
  name: string;
  sub: string;
  path: string | null;
  problem: boolean;
  windows: boolean;
  control: ReactNode;
}

const START: Record<ServiceItem['startType'], string> = {
  boot: 'Boot',
  system: 'System',
  automatic: 'Automatic',
  manual: 'Manual',
  disabled: 'Disabled',
};

const START_OPTIONS: { value: ServiceStart; label: string }[] = [
  { value: 'automatic', label: 'Automatic' },
  { value: 'delayed', label: 'Automatic (delayed)' },
  { value: 'manual', label: 'Manual' },
  { value: 'disabled', label: 'Disabled' },
];

const HINT: Record<Exclude<Tab, 'drivers'>, string> = {
  startup:
    'Programs that start when you sign in, from the Run keys and Startup folders. The switch is the same one Task Manager uses – the entry itself stays.',
  services: 'Services registered on this PC and how they start. «Manual» services start only when an app asks for them.',
  tasks: 'Scheduled tasks. A task that is switched off stays in place and can be switched back on at any time.',
};

const ICON: Record<Tab, IconName> = { startup: 'power', services: 'cog', tasks: 'calendar', drivers: 'cpu' };

const WINDOWS_HINT = 'Part of Windows – UWiper leaves it as is';
const ADMIN_HINT = 'Needs administrator rights';

/** A state that can't be changed here, shown as a label instead of a control. */
function Locked({ tone, label, hint }: { tone: PillTone; label: string; hint: string }) {
  return (
    <span title={hint}>
      <Pill tone={tone}>{label}</Pill>
    </span>
  );
}

export function System() {
  const env = useAppStore((s) => s.env);
  const safety = useAppStore((s) => s.settings?.safety);
  const attempt = useAppStore((s) => s.attempt);
  const notify = useAppStore((s) => s.notify);
  const update = useAppStore((s) => s.update);
  const refresh = useAppStore((s) => s.refresh);
  const afterChange = useAppStore((s) => s.afterChange);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const startup = useResource('startup');
  const services = useResource('services');
  const tasks = useResource('tasks');
  const drivers = useResource('drivers');
  const [tab, setTab] = useState<Tab>('startup');
  const [hideWindows, setHideWindows] = useState(true);
  const [problemsOnly, setProblemsOnly] = useState(false);
  const [oldOnly, setOldOnly] = useState(false);
  const [busy, setBusy] = useState<ReadonlySet<string>>(new Set());
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [problems, setProblems] = useState<Problem[]>([]);

  const elevated = !!env?.elevated;
  const quarantine = safety?.quarantine ?? true;

  // Overview's "Start with Windows" opens the Startup tab.
  const focus = useFocus(true, false);
  useEffect(() => {
    if (focus && (TABS as string[]).includes(focus)) setTab(focus as Tab);
  }, [focus]);

  /** Shows the change at once, then reads the list again: that shows what Windows really has, also when it refused. */
  const change = async <T,>(key: ResourceKey, id: string, is: (i: T) => boolean, next: Partial<T>, run: () => Promise<void>) => {
    update(key, (list) => (list as T[]).map((i) => (is(i) ? { ...i, ...next } : i)) as never);
    setBusy((b) => new Set(b).add(id));
    try {
      await run();
    } catch (e) {
      notify(errorText(e));
    }
    refresh(key);
    setBusy((b) => {
      const rest = new Set(b);
      rest.delete(id);
      return rest;
    });
  };

  const missing = <Pill tone="bad">File missing</Pill>;

  const rows: Row[] =
    tab === 'startup'
      ? (startup.data ?? []).map((i) => ({
          key: i.id,
          name: i.name,
          sub: i.location,
          path: i.target ?? i.command,
          problem: i.fileExists === false,
          windows: i.isWindows,
          control:
            i.fileExists === false ? (
              missing
            ) : i.isWindows ? (
              <Locked tone={i.enabled ? 'good' : 'dim'} label={i.enabled ? 'On' : 'Off'} hint={WINDOWS_HINT} />
            ) : (
              <span title={i.needsAdmin && !elevated ? ADMIN_HINT : undefined} className="flex">
                <Switch
                  small
                  checked={i.enabled}
                  disabled={busy.has(i.id) || (i.needsAdmin && !elevated)}
                  label={`Start ${i.name} with Windows`}
                  onChange={(on) => change<typeof i>('startup', i.id, (x) => x.id === i.id, { enabled: on }, () => api.setStartupEnabled(i.id, on))}
                />
              </span>
            ),
        }))
      : tab === 'services'
        ? (services.data ?? []).map((s) => {
            const start: ServiceStart | null =
              s.startType === 'automatic' ? (s.delayed ? 'delayed' : 'automatic') : s.startType === 'manual' || s.startType === 'disabled' ? s.startType : null;
            return {
              key: s.name,
              name: s.displayName,
              sub: s.name,
              path: s.binary,
              problem: s.fileExists === false,
              windows: s.isWindows,
              control:
                s.fileExists === false ? (
                  missing
                ) : s.isWindows || !start ? (
                  <Locked
                    tone={s.startType === 'automatic' ? 'good' : 'dim'}
                    label={`${START[s.startType]}${s.delayed ? ' (delayed)' : ''}`}
                    hint={WINDOWS_HINT}
                  />
                ) : (
                  <Select
                    label={`How ${s.displayName} starts`}
                    title={elevated ? undefined : ADMIN_HINT}
                    value={start}
                    options={START_OPTIONS}
                    disabled={busy.has(s.name) || !elevated}
                    className="w-[164px]"
                    onChange={(next) =>
                      change<typeof s>(
                        'services',
                        s.name,
                        (x) => x.name === s.name,
                        { startType: next === 'delayed' ? 'automatic' : next, delayed: next === 'delayed' },
                        () => api.setServiceStart(s.name, next),
                      )
                    }
                  />
                ),
            };
          })
        : tab === 'tasks'
          ? (tasks.data ?? []).map((t) => ({
              key: t.name,
              name: t.name,
              sub: t.triggers,
              path: t.binary ?? t.command ?? 'COM handler',
              problem: t.fileExists === false,
              windows: t.isWindows,
              control:
                t.fileExists === false ? (
                  missing
                ) : t.isWindows ? (
                  <Locked tone={t.enabled ? 'good' : 'dim'} label={t.enabled ? 'On' : 'Off'} hint={WINDOWS_HINT} />
                ) : (
                  <Switch
                    small
                    checked={t.enabled}
                    disabled={busy.has(t.name)}
                    label={`Run ${t.name}`}
                    onChange={(on) => change<typeof t>('tasks', t.name, (x) => x.name === t.name, { enabled: on }, () => api.setTaskEnabled(t.name, on))}
                  />
                ),
            }))
          : [];

  const visible = rows
    .filter((r) => !(hideWindows && r.windows) && (!problemsOnly || r.problem))
    .sort((a, b) => Number(b.problem) - Number(a.problem));
  const lockedForAdmin =
    !elevated &&
    ((tab === 'startup' && (startup.data ?? []).some((i) => i.needsAdmin && !i.isWindows && i.fileExists !== false)) ||
      (tab === 'services' && visible.some((r) => !r.windows && !r.problem)));

  // Drivers: only older versions that are surely spare are picked to start with.
  const driverItems = drivers.data ?? [];
  const canRemove = (d: DriverItem) => elevated && d.usage === 'unused';
  useEffect(() => {
    setPicked(new Set((drivers.data ?? []).filter((d) => d.suggested && elevated).map((d) => d.id)));
  }, [drivers.data, elevated]);
  const shownDrivers = oldOnly ? driverItems.filter((d) => d.old) : driverItems;
  const chosen = driverItems.filter((d) => picked.has(d.id));
  const chosenBytes = chosen.reduce((s, d) => s + (d.sizeBytes ?? 0), 0);
  const oldBytes = driverItems.filter((d) => d.suggested).reduce((s, d) => s + (d.sizeBytes ?? 0), 0);

  const removeDrivers = async () => {
    setConfirming(false);
    setRemoving(true);
    setProblems([]);
    try {
      const report = await api.removeItems(
        'Old drivers',
        chosen.map((d) => ({ label: `${d.device ?? d.provider} ${d.version ?? ''} (${d.id})`.replace('  ', ' '), target: d.removal, sizeBytes: d.sizeBytes ?? 0, check: d.check })),
      );
      setProblems(report.problems);
      if (report.removed > 0) {
        const what = `${plural(report.removed, 'driver package', 'driver packages')}${report.freedBytes ? ` (${formatBytes(report.freedBytes)})` : ''}`;
        notify(report.batchId ? `${what} moved to quarantine` : `${what} deleted`);
      }
      if (report.notes.length) notify(report.notes.join(' '));
    } catch (e) {
      notify(errorText(e));
    }
    setRemoving(false);
    afterChange();
  };

  const countOf = (r: { data: { isWindows: boolean }[] | null; error: string | null }) =>
    r.data ? <SegmentCount>{r.data.filter((i) => !(hideWindows && i.isWindows)).length}</SegmentCount> : !r.error && <SegmentCount>…</SegmentCount>;
  const resource = tab === 'startup' ? startup : tab === 'services' ? services : tab === 'tasks' ? tasks : null;
  const adminCallout = (message: string) => <Callout message={message} action="Restart as administrator" onAction={restartAsAdmin} />;

  return (
    <>
      <PageHeader title="System" subtitle="Everything that runs without asking: startup items, services, scheduled tasks and drivers">
        {tab === 'drivers' && driverItems.some(canRemove) && (
          <Button
            kind={quarantine ? 'primary' : 'danger'}
            icon={quarantine ? 'archive' : 'trash'}
            disabled={chosen.length === 0 || drivers.loading}
            busy={removing}
            onClick={() => (quarantine ? removeDrivers() : setConfirming(true))}
          >
            {quarantine ? `Move ${chosen.length} to quarantine` : `Delete ${chosen.length}`}
          </Button>
        )}
      </PageHeader>
      <Toolbar>
        <Segmented<Tab>
          label="System areas"
          value={tab}
          onChange={setTab}
          options={[
            { value: 'startup', label: <>Startup {countOf(startup)}</> },
            { value: 'services', label: <>Services {countOf(services)}</> },
            { value: 'tasks', label: <>Scheduled tasks {countOf(tasks)}</> },
            {
              value: 'drivers',
              label: <>Drivers {drivers.data ? <SegmentCount>{drivers.data.length}</SegmentCount> : !drivers.error && <SegmentCount>…</SegmentCount>}</>,
            },
          ]}
        />
        <div className="flex-1" />
        {tab === 'drivers' ? (
          <label className="inline-flex items-center gap-2 text-meta text-fg-2">
            Older versions only
            <Switch small checked={oldOnly} onChange={setOldOnly} label="Older versions only" />
          </label>
        ) : (
          <>
            <label className="inline-flex items-center gap-2 text-meta text-fg-2">
              Hide Windows items
              <Switch small checked={hideWindows} onChange={setHideWindows} label="Hide Windows items" />
            </label>
            <label className="inline-flex items-center gap-2 text-meta text-fg-2">
              Problems only
              <Switch small checked={problemsOnly} onChange={setProblemsOnly} label="Problems only" />
            </label>
          </>
        )}
      </Toolbar>

      <PageBody>
        {tab === 'drivers' || !resource ? (
          <>
            <Note>
              Driver packages that apps and Windows Update added. Each update leaves the previous version in the Driver Store, and
              Windows never removes it. UWiper picks <strong>older versions</strong> nothing uses while a device runs the newest one.
              Printer drivers and packages a device, service or printer still uses are never picked.
            </Note>
            {drivers.error && <Callout message={drivers.error} action="Try again" onAction={drivers.reload} />}
            {!elevated && driverItems.length > 0 && adminCallout('Removing driver packages needs administrator rights.')}
            <ProblemList title={`${plural(problems.length, "package wasn't", "packages weren't")} removed`} problems={problems} />
            {drivers.loading && !drivers.data && <SkeletonRows rows={6} />}
            {drivers.data && shownDrivers.length === 0 && (
              <Empty icon="check" title={oldOnly ? 'No older versions' : 'No third-party drivers'}>
                {oldOnly ? 'Every driver package here is the newest version of itself.' : 'Only the drivers that come with Windows are installed.'}
              </Empty>
            )}
            {shownDrivers.length > 0 && (
              <>
                <DriverList items={shownDrivers} selected={picked} onChange={setPicked} canRemove={canRemove} />
                <div className="text-right text-small text-fg-3">
                  {chosen.length > 0
                    ? `${plural(chosen.length, 'package', 'packages')} selected · ${formatBytes(chosenBytes)}`
                    : oldBytes > 0
                      ? `${formatBytes(oldBytes)} in older versions nothing uses`
                      : null}
                </div>
              </>
            )}
          </>
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
            {lockedForAdmin &&
              adminCallout(
                tab === 'services'
                  ? 'Changing how services start needs administrator rights.'
                  : 'Entries that start for every user can only be switched with administrator rights.',
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
                    {/* Fixed width, so the path column lines up across rows. */}
                    <span className="flex w-[204px] flex-none items-center justify-end gap-1.5">
                      {r.control}
                      {r.path && !r.problem && r.path.includes('\\') ? (
                        <Button small kind="ghost" icon="folder" className="w-[30px] px-0" ariaLabel={`Show ${r.name} in Explorer`} onClick={() => attempt(() => api.revealPath(r.path!))} />
                      ) : (
                        <span className="w-[30px]" />
                      )}
                    </span>
                  </ListRow>
                ))}
              </ListCard>
            )}
          </>
        )}
      </PageBody>

      {confirming && (
        <Dialog
          label="Delete driver packages"
          onClose={() => setConfirming(false)}
          footer={
            <>
              <Button onClick={() => setConfirming(false)}>Cancel</Button>
              <Button kind="danger" icon="trash" onClick={removeDrivers}>
                Delete for good
              </Button>
            </>
          }
        >
          <h2 className="text-h2 font-bold">Delete {plural(chosen.length, 'driver package', 'driver packages')} for good?</h2>
          <p className="mt-2 text-body leading-normal text-fg-3">
            Quarantine is off in Settings, so these can't be put back from UWiper. Turn it on to keep a copy of every package UWiper
            removes.
          </p>
        </Dialog>
      )}
    </>
  );
}
