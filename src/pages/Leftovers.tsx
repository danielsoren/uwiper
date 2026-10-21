import { PageBody, PageHeader } from '../components/layout/Page';
import { plural } from '../lib/format';
import { useResource } from '../store/hooks';
import { brokenEntries } from '../store/selectors';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Empty, GroupTitle, IconTile, ListCard, ListRow, Loading, Note, Pill, type IconName } from '../ui';

interface Row {
  key: string;
  name: string;
  path: string | null;
  why: string;
}

export function Leftovers() {
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const startup = useResource('startup');
  const services = useResource('services');
  const tasks = useResource('tasks');

  const loading = startup.loading || services.loading || tasks.loading;
  const ready = startup.data !== null || services.data !== null || tasks.data !== null;
  const broken = brokenEntries(startup.data, services.data, tasks.data);

  const groups: { title: string; icon: IconName; rows: Row[]; error: string | null }[] = [
    {
      title: 'Start with Windows',
      icon: 'power',
      error: startup.error,
      rows: broken.startup.map((i) => ({ key: i.id, name: i.name, path: i.target, why: `${i.location} – the program it starts is gone` })),
    },
    {
      title: 'Services',
      icon: 'cog',
      error: services.error,
      rows: broken.services.map((s) => ({ key: s.name, name: s.displayName, path: s.binary, why: `Service “${s.name}” – its executable is gone` })),
    },
    {
      title: 'Scheduled tasks',
      icon: 'calendar',
      error: tasks.error,
      rows: broken.tasks.map((t) => ({ key: t.name, name: t.name, path: t.binary, why: `${t.triggers} – runs a file that no longer exists` })),
    },
  ];

  return (
    <>
      <PageHeader
        title="Leftovers"
        subtitle={ready ? `${plural(broken.total, 'entry', 'entries')} left behind by apps that are already gone` : 'Looking for entries left behind by removed apps…'}
      >
        <Button icon="refresh" onClick={() => [startup, services, tasks].forEach((r) => r.reload())}>
          Rescan
        </Button>
      </PageHeader>
      <PageBody>
        <Note>
          This version finds startup entries, services and scheduled tasks that point to files that no longer exist. Matching
          leftover folders and registry keys to the apps that created them arrives in v0.2, together with removing them.
        </Note>

        {!ready && loading && <Loading label="Checking startup entries, services and scheduled tasks…" />}
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
                      leading={<IconTile icon={g.icon} tone="orange" small />}
                      title={r.name}
                      subtitle={
                        <>
                          {r.path && <span className="block select-text break-all font-mono text-fg-2">{r.path}</span>}
                          {r.why}
                        </>
                      }
                    >
                      <Pill tone="bad">File missing</Pill>
                    </ListRow>
                  ))}
                </ListCard>
              )}
            </section>
          ))}
      </PageBody>
    </>
  );
}
