import { PageBody, PageHeader } from '../components/layout/Page';
import { api } from '../lib/api';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Card, Empty, IconTile } from '../ui';

export function Quarantine() {
  const go = useAppStore((s) => s.go);
  const attempt = useAppStore((s) => s.attempt);
  const keepDays = useAppStore((s) => s.settings?.safety.keepDays ?? 30);
  const storage = useResource('storage');
  const dir = storage.data?.dirs.find((d) => d.key === 'quarantine');

  return (
    <>
      <PageHeader
        title="Quarantine"
        subtitle={`Everything UWiper removes waits here, ready to put back. Each batch is deleted for good after ${keepDays} days.`}
      />
      <PageBody>
        <Card padded className="flex flex-wrap items-center gap-4">
          <IconTile icon="folder" />
          <div className="min-w-0 flex-1">
            <div className="text-label font-semibold">Quarantine folder</div>
            <div className="mt-1 select-text break-all font-mono text-small text-fg-path">{dir?.path ?? '…'}</div>
            {dir && !dir.inside && <div className="text-small text-fg-3">Outside the UWiper folder</div>}
          </div>
          <Button
            small
            disabled={!dir?.exists}
            title={dir?.exists ? undefined : 'Created when something is first quarantined'}
            onClick={() => dir && attempt(() => api.openFolder(dir.path))}
          >
            Open
          </Button>
          <Button small kind="ghost" onClick={() => go('settings')}>
            Change
          </Button>
        </Card>
        <Empty icon="archive" title="Quarantine is empty">
          This version doesn't remove anything yet. From v0.2 on, uninstall leftovers will land here first – files together with
          .reg backups of every deleted registry key – so any of it can be restored in one click.
        </Empty>
      </PageBody>
    </>
  );
}
