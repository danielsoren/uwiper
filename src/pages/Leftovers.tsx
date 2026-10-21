import { useEffect } from 'react';
import { Icon, type IconName } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Empty, ErrorBox, GroupTitle, Loading, Note, Pill } from '../components/ui';
import { plural } from '../lib/format';
import { brokenEntries, useStore } from '../state/store';

interface Row {
  key: string;
  name: string;
  path: string | null;
  why: string;
}

export function Leftovers() {
  const store = useStore();
  const { startup, services, tasks, env } = store;

  useEffect(() => {
    startup.load();
    services.load();
    tasks.load();
  }, [startup.load, services.load, tasks.load]);

  const loading = startup.loading || services.loading || tasks.loading;
  const ready = startup.data !== null || services.data !== null || tasks.data !== null;
  const broken = brokenEntries(store);

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
        <Button
          icon="refresh"
          onClick={() => {
            startup.load(true);
            services.load(true);
            tasks.load(true);
          }}
        >
          Rescan
        </Button>
      </PageHeader>
      <div className="page-body">
        <Note>
          This version finds startup entries, services and scheduled tasks that point to files that no longer exist.
          Matching leftover folders and registry keys to the apps that created them arrives in v0.2, together with
          removing them.
        </Note>

        {!ready && loading && <Loading label="Checking startup entries, services and scheduled tasks…" />}

        {ready && broken.total === 0 && !loading && (
          <Empty icon="check" title="Nothing pointing nowhere">
            Every startup entry, service and scheduled task we could read points to a file that exists.
          </Empty>
        )}

        {groups.map((g) =>
          g.rows.length > 0 || g.error ? (
            <section key={g.title}>
              <GroupTitle aside={g.rows.length ? plural(g.rows.length, 'entry', 'entries') : undefined}>{g.title}</GroupTitle>
              {g.error ? (
                <ErrorBox
                  message={g.error}
                  onRetry={g.error.includes('administrator') && env?.windows ? store.restartAsAdmin : undefined}
                  retryLabel="Restart as administrator"
                />
              ) : (
                <div className="list-card">
                  {g.rows.map((r) => (
                    <div className="item-row" key={r.key}>
                      <span className="item-icon tone-orange">
                        <Icon name={g.icon} size={17} />
                      </span>
                      <span className="item-text">
                        <span className="item-name">{r.name}</span>
                        {r.path && <span className="mono item-path">{r.path}</span>}
                        <span className="item-sub">{r.why}</span>
                      </span>
                      <Pill tone="bad">File missing</Pill>
                    </div>
                  ))}
                </div>
              )}
            </section>
          ) : null,
        )}
      </div>
    </>
  );
}
