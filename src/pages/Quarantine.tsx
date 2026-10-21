import { useEffect } from 'react';
import { Icon } from '../components/Icon';
import { PageHeader } from '../components/PageHeader';
import { Button, Card, Empty } from '../components/ui';
import { api } from '../lib/api';
import { useStore } from '../state/store';

export function Quarantine() {
  const { storage, settings, go, attempt } = useStore();
  useEffect(() => storage.load(), [storage.load]);

  const dir = storage.data?.dirs.find((d) => d.key === 'quarantine');
  const keepDays = settings?.safety.keepDays ?? 30;

  return (
    <>
      <PageHeader
        title="Quarantine"
        subtitle={`Everything UWiper removes waits here, ready to put back. Each batch is deleted for good after ${keepDays} days.`}
      />
      <div className="page-body">
        <Card className="pad location-card">
          <span className="stat-icon tone-accent">
            <Icon name="folder" />
          </span>
          <div className="min-0 flex-1">
            <div className="item-name">Quarantine folder</div>
            <div className="mono path-line">{dir?.path ?? '…'}</div>
            {dir && !dir.inside && <div className="item-sub">Outside the UWiper folder</div>}
          </div>
          <Button small disabled={!dir?.exists} title={dir?.exists ? undefined : 'Created when something is first quarantined'} onClick={() => dir && attempt(() => api.openFolder(dir.path))}>
            Open
          </Button>
          <Button small kind="ghost" onClick={() => go('settings')}>
            Change
          </Button>
        </Card>
        <Empty icon="archive" title="Quarantine is empty">
          This version doesn't remove anything yet. From v0.2 on, uninstall leftovers will land here first – files
          together with .reg backups of every deleted registry key – so any of it can be restored in one click.
        </Empty>
      </div>
    </>
  );
}
