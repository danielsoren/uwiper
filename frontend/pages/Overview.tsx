import { CacheBreakdown } from '../components/cleanup/CacheBreakdown';
import { PageBody, PageHeader } from '../components/layout/Page';
import { StatCard } from '../components/overview/StatCard';
import { api } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import { useResource } from '../store/hooks';
import { brokenEntries } from '../store/selectors';
import { useAppStore } from '../store/useAppStore';
import { Button, Card, Icon, IconTile, Skeleton } from '../ui';

function BigNumber({ bytes }: { bytes: number }) {
  const [value, unit] = formatBytes(bytes).split(' ');
  return (
    <div className="mt-2 text-hero font-bold leading-none tracking-[-0.035em]">
      {value} <span className="text-[28px] font-semibold tracking-[-0.01em] text-fg-3">{unit}</span>
    </div>
  );
}

export function Overview() {
  const go = useAppStore((s) => s.go);
  const env = useAppStore((s) => s.env);
  const settings = useAppStore((s) => s.settings);
  const attempt = useAppStore((s) => s.attempt);
  const cleanup = useResource('cleanup');
  const apps = useResource('apps');
  const startup = useResource('startup');
  const services = useResource('services');
  const tasks = useResource('tasks');
  const storage = useResource('storage');

  const rescan = () => [cleanup, apps, startup, services, tasks, storage].forEach((r) => r.reload());

  const items = cleanup.data ?? [];
  const safeBytes = items.filter((i) => i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const optionalBytes = items.filter((i) => !i.defaultOn).reduce((s, i) => s + i.sizeBytes, 0);
  const broken = brokenEntries(startup.data, services.data, tasks.data);
  const appBytes = (apps.data ?? []).reduce((s, a) => s + (a.sizeBytes ?? 0), 0);
  const outside = (storage.data?.dirs ?? []).filter((d) => !d.inside).length + (settings?.footprint.startWithWindows ? 1 : 0);

  return (
    <>
      <PageHeader title="Overview" subtitle="A read-only look at your PC – this version shows, it doesn't remove anything yet">
        <Button icon="refresh" onClick={rescan} busy={[cleanup, apps, startup, services, tasks].some((r) => r.loading)}>
          Scan again
        </Button>
      </PageHeader>
      <PageBody>
        <Card className="flex flex-wrap items-center gap-7 px-7 py-[26px]">
          <div className="min-w-0 flex-[1_1_280px]">
            <h2 className="text-small font-semibold uppercase tracking-[.06em] text-fg-3">Caches you could free</h2>
            {cleanup.loading && !cleanup.data ? (
              <Skeleton className="mt-2 h-14 w-52 rounded-xl" />
            ) : (
              <BigNumber bytes={safeBytes} />
            )}
            <p className="mb-5 mt-3 max-w-[420px] text-body leading-normal text-[#b4b4bb]">
              Temporary files and caches that Windows and your apps rebuild on their own.
              {optionalBytes > 0 && ` Plus ${formatBytes(optionalBytes)} of developer and app caches that stay off unless you pick them.`}
            </p>
            <Button kind="primary" icon="sparkle" onClick={() => go('cleanup')}>
              See what's inside
            </Button>
          </div>
          <CacheBreakdown items={items} onlyDefault onPick={() => go('cleanup')} />
        </Card>

        <div className="flex flex-wrap gap-3">
          <StatCard
            icon="apps"
            tone="accent"
            value={apps.data ? String(apps.data.length) : null}
            loading={apps.loading}
            label="Installed apps"
            sub={apps.error ?? (apps.data ? `${formatBytes(appBytes)} on disk` : 'Measuring sizes…')}
            onClick={() => go('apps')}
          />
          <StatCard
            icon="traces"
            tone="purple"
            value={startup.data || services.data || tasks.data ? String(broken.total) : null}
            loading={startup.loading || services.loading || tasks.loading}
            label="Entries pointing nowhere"
            sub="Startup, services and tasks whose files are gone"
            onClick={() => go('leftovers')}
          />
          <StatCard
            icon="power"
            tone="orange"
            value={startup.data ? String(startup.data.filter((i) => i.enabled).length) : null}
            loading={startup.loading}
            label="Start with Windows"
            sub={startup.error ?? (startup.data ? `${plural(startup.data.length, 'entry', 'entries')} in total` : 'Reading…')}
            onClick={() => go('system')}
          />
          <StatCard icon="archive" tone="green" value="Empty" label="Quarantine" sub="Nothing has been removed yet" onClick={() => go('quarantine')} />
        </div>

        <div className="flex flex-wrap gap-4 [&>*]:min-w-0 [&>*]:flex-[1_1_320px]">
          <Card padded>
            <div className="mb-3 flex items-center gap-3">
              <IconTile icon="folder" />
              <h2 className="text-title font-semibold">Where UWiper lives</h2>
            </div>
            <p className="mb-1.5 select-text break-all font-mono text-small text-fg-path">{env?.home ?? '…'}</p>
            <p className="mb-3.5 text-small leading-normal text-fg-3">
              {outside === 0
                ? 'Everything UWiper keeps is in this one folder. Nothing in AppData, ProgramData or the registry.'
                : `${plural(outside, 'thing', 'things')} outside this folder, all listed in Settings.`}
            </p>
            <div className="flex flex-wrap gap-2.5">
              <Button small icon="folder" onClick={() => env && attempt(() => api.openFolder(env.home))}>
                Open folder
              </Button>
              <Button small kind="ghost" onClick={() => go('settings')}>
                Storage settings
              </Button>
            </div>
          </Card>
          <Card padded>
            <div className="mb-3 flex items-center gap-3">
              <IconTile icon="shield" tone="green" />
              <h2 className="text-title font-semibold">Safe by design</h2>
            </div>
            <ul className="flex flex-col gap-2.5">
              {[
                'This version only reads – it never changes your system',
                `Removals will go to quarantine first, restorable for ${settings?.safety.keepDays ?? 30} days`,
                'Windows components are never offered for removal',
              ].map((line) => (
                <li key={line} className="flex items-start gap-2.5 text-meta leading-[1.45] text-fg-2">
                  <Icon name="check" size={16} stroke={2.2} className="mt-px text-good" />
                  {line}
                </li>
              ))}
            </ul>
          </Card>
        </div>
      </PageBody>
    </>
  );
}
