import { useState } from 'react';
import { PageBody, PageHeader } from '../components/layout/Page';
import { ProblemList } from '../components/leftovers/ProblemList';
import { api, errorText } from '../lib/api';
import { plural } from '../lib/format';
import type { Problem, Target } from '../lib/types';
import { useResource } from '../store/hooks';
import { brokenEntries } from '../store/selectors';
import { useAppStore } from '../store/useAppStore';
import {
  Button,
  Callout,
  Checkbox,
  Dialog,
  Empty,
  GroupTitle,
  IconTile,
  ListCard,
  ListRow,
  Note,
  Pill,
  SkeletonRows,
  type IconName,
} from '../ui';

interface Row {
  key: string;
  name: string;
  path: string | null;
  why: string;
  removal: Target;
  /** Services, tasks and machine-wide startup entries need administrator rights. */
  admin: boolean;
}

export function Leftovers() {
  const env = useAppStore((s) => s.env);
  const keepDays = useAppStore((s) => s.settings?.safety.keepDays ?? 30);
  const quarantine = useAppStore((s) => s.settings?.safety.quarantine ?? true);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const afterChange = useAppStore((s) => s.afterChange);
  const notify = useAppStore((s) => s.notify);
  const startup = useResource('startup');
  const services = useResource('services');
  const tasks = useResource('tasks');
  // Everything is selected unless unticked.
  const [unticked, setUnticked] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [problems, setProblems] = useState<Problem[]>([]);

  const loading = startup.loading || services.loading || tasks.loading;
  const ready = startup.data !== null || services.data !== null || tasks.data !== null;
  const broken = brokenEntries(startup.data, services.data, tasks.data);
  const elevated = !!env?.elevated;

  const groups: { title: string; icon: IconName; rows: Row[]; error: string | null }[] = [
    {
      title: 'Start with Windows',
      icon: 'power',
      error: startup.error,
      rows: broken.startup.map((i) => ({
        key: i.id,
        name: i.name,
        path: i.target,
        why: `${i.location} – the program it starts is gone`,
        removal: i.removal,
        admin: i.removal.kind === 'registryValue' ? i.removal.key.startsWith('HKLM') : !i.target?.toLowerCase().includes('\\users\\'),
      })),
    },
    {
      title: 'Services',
      icon: 'cog',
      error: services.error,
      rows: broken.services.map((s) => ({
        key: s.name,
        name: s.displayName,
        path: s.binary,
        why: `Service “${s.name}” – its executable is gone`,
        removal: s.removal,
        admin: true,
      })),
    },
    {
      title: 'Scheduled tasks',
      icon: 'calendar',
      error: tasks.error,
      rows: broken.tasks.map((t) => ({
        key: t.name,
        name: t.name,
        path: t.binary,
        why: `${t.triggers} – runs a file that no longer exists`,
        removal: t.removal,
        admin: true,
      })),
    },
  ];

  const rows = groups.flatMap((g) => g.rows);
  const removable = (r: Row) => elevated || !r.admin;
  const picked = rows.filter((r) => removable(r) && !unticked.has(r.key));
  const needAdmin = rows.filter((r) => !removable(r)).length;

  const toggle = (key: string, on: boolean) =>
    setUnticked((u) => {
      const next = new Set(u);
      if (on) next.delete(key);
      else next.add(key);
      return next;
    });

  const remove = async () => {
    setConfirming(false);
    setRemoving(true);
    setProblems([]);
    try {
      const report = await api.removeItems(
        'Leftover entries',
        picked.map((r) => ({ label: r.name, target: r.removal })),
      );
      setProblems(report.problems);
      if (report.removed > 0) {
        notify(
          report.batchId
            ? `${plural(report.removed, 'entry', 'entries')} moved to quarantine`
            : `${plural(report.removed, 'entry', 'entries')} deleted`,
        );
      }
      if (report.notes.length) notify(report.notes.join(' '));
    } catch (e) {
      notify(errorText(e));
    }
    setRemoving(false);
    afterChange();
  };

  return (
    <>
      <PageHeader
        title="Leftovers"
        subtitle={ready ? `${plural(broken.total, 'entry', 'entries')} left behind by apps that are already gone` : 'Looking for entries left behind by removed apps…'}
      >
        <Button icon="refresh" onClick={() => [startup, services, tasks].forEach((r) => r.reload())} busy={loading}>
          Rescan
        </Button>
        {broken.total > 0 && (
          <Button
            kind={quarantine ? 'primary' : 'danger'}
            icon={quarantine ? 'archive' : 'trash'}
            disabled={picked.length === 0}
            busy={removing}
            onClick={() => (quarantine ? remove() : setConfirming(true))}
          >
            {quarantine ? `Move ${picked.length} to quarantine` : `Delete ${picked.length}`}
          </Button>
        )}
      </PageHeader>
      <PageBody>
        <Note>
          Startup entries, services and scheduled tasks that point to files that no longer exist.
          {quarantine ? ` Removed entries stay in quarantine for ${keepDays} days, ready to put back.` : ' Quarantine is off, so removing deletes them for good.'}{' '}
          Leftover folders and registry keys are found when you uninstall an app from Apps.
        </Note>

        {needAdmin > 0 && (
          <Callout
            message={`${plural(needAdmin, 'entry needs', 'entries need')} administrator rights to remove.`}
            action="Restart as administrator"
            onAction={restartAsAdmin}
          />
        )}
        <ProblemList title={`${plural(problems.length, "entry wasn't", "entries weren't")} removed`} problems={problems} />

        {!ready && loading && <SkeletonRows rows={4} />}
        {ready && !loading && broken.total === 0 && (
          <Empty icon="check" title="Nothing pointing nowhere">
            Every startup entry, service and scheduled task we could read points to a file that exists.
          </Empty>
        )}

        {groups
          .filter((g) => g.rows.length > 0 || g.error)
          .map((g) => (
            <section key={g.title}>
              <GroupTitle aside={g.rows.length ? plural(g.rows.length, 'entry', 'entries') : undefined}>{g.title}</GroupTitle>
              {g.error ? (
                <Callout
                  message={g.error}
                  {...(g.error.includes('administrator') ? { action: 'Restart as administrator', onAction: restartAsAdmin } : {})}
                />
              ) : (
                <ListCard>
                  {g.rows.map((r) => (
                    <ListRow
                      key={r.key}
                      leading={
                        <>
                          <Checkbox
                            label={`Remove ${r.name}`}
                            checked={removable(r) && !unticked.has(r.key)}
                            disabled={!removable(r)}
                            onChange={(on) => toggle(r.key, on)}
                          />
                          <IconTile icon={g.icon} tone="orange" small />
                        </>
                      }
                      title={r.name}
                      subtitle={
                        <>
                          {r.path && <span className="block select-text break-all font-mono text-fg-2">{r.path}</span>}
                          {r.why}
                        </>
                      }
                    >
                      {!removable(r) && <Pill tone="warn">Needs admin</Pill>}
                      <Pill tone="bad">File missing</Pill>
                    </ListRow>
                  ))}
                </ListCard>
              )}
            </section>
          ))}
      </PageBody>

      {confirming && (
        <Dialog
          label="Delete entries"
          onClose={() => setConfirming(false)}
          footer={
            <>
              <Button onClick={() => setConfirming(false)}>Cancel</Button>
              <Button kind="danger" icon="trash" onClick={remove}>
                Delete for good
              </Button>
            </>
          }
        >
          <h2 className="text-h2 font-bold">Delete {plural(picked.length, 'entry', 'entries')} for good?</h2>
          <p className="mt-2 text-body leading-normal text-fg-3">
            Quarantine is off in Settings, so these can't be put back. Turn it on to keep a copy of everything UWiper removes.
          </p>
        </Dialog>
      )}
    </>
  );
}
