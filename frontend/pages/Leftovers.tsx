import { useEffect, useState } from 'react';
import { PageBody, PageHeader } from '../components/layout/Page';
import { LeftoverReview, preselect } from '../components/leftovers/LeftoverReview';
import { ProblemList } from '../components/leftovers/ProblemList';
import { api, errorText } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { Leftover, Problem } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Dialog, Empty, Note, SkeletonRows } from '../ui';

export function Leftovers() {
  const env = useAppStore((s) => s.env);
  const safety = useAppStore((s) => s.settings?.safety);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const afterChange = useAppStore((s) => s.afterChange);
  const refresh = useAppStore((s) => s.refresh);
  const notify = useAppStore((s) => s.notify);
  const orphans = useResource('orphans');
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [problems, setProblems] = useState<Problem[]>([]);

  const elevated = !!env?.elevated;
  const quarantine = safety?.quarantine ?? true;
  const items = orphans.data?.items ?? [];
  const canRemove = (l: Leftover) => elevated || !l.needsAdmin;

  // A new scan starts from what Settings says to pick.
  useEffect(() => {
    setSelected(preselect(orphans.data?.items ?? [], safety?.preselect ?? 'high-medium', (l) => elevated || !l.needsAdmin));
  }, [orphans.data, safety?.preselect, elevated]);

  const picked = items.filter((l) => selected.has(l.id));
  const bytes = picked.reduce((s, l) => s + (l.sizeBytes ?? 0), 0);
  const blocked = items.filter((l) => !canRemove(l)).length;
  const sure = items.filter((l) => l.confidence !== 'low').length;

  const remove = async () => {
    setConfirming(false);
    setRemoving(true);
    setProblems([]);
    try {
      const report = await api.removeItems(
        'Leftovers',
        picked.map((l) => ({ label: l.label, target: l.target, sizeBytes: l.sizeBytes ?? 0 })),
      );
      setProblems(report.problems);
      if (report.removed > 0) {
        const what = `${plural(report.removed, 'item', 'items')}${report.freedBytes ? ` (${formatBytes(report.freedBytes)})` : ''}`;
        notify(report.batchId ? `${what} moved to quarantine` : `${what} deleted`);
      }
      if (report.notes.length) notify(report.notes.join(' '));
    } catch (e) {
      notify(errorText(e));
    }
    setRemoving(false);
    afterChange();
    // Removing an "installed" app's entry changes the Apps list too.
    if (picked.some((l) => l.kind === 'app')) refresh('apps');
  };

  return (
    <>
      <PageHeader
        title="Leftovers"
        subtitle={
          orphans.data
            ? items.length
              ? `${plural(sure, 'thing', 'things')} left behind by apps that are gone${items.length > sure ? `, ${items.length - sure} more that might be` : ''}`
              : 'Nothing left behind by apps that are gone'
            : 'Looking for what removed apps left behind…'
        }
      >
        <Button icon="refresh" onClick={orphans.reload} busy={orphans.loading}>
          Rescan
        </Button>
        {items.length > 0 && (
          <Button
            kind={quarantine ? 'primary' : 'danger'}
            icon={quarantine ? 'archive' : 'trash'}
            disabled={picked.length === 0 || orphans.loading}
            busy={removing}
            onClick={() => (quarantine ? remove() : setConfirming(true))}
          >
            {quarantine ? `Move ${picked.length} to quarantine` : `Delete ${picked.length}`}
          </Button>
        )}
      </PageHeader>
      <PageBody>
        <Note>
          What apps leave behind once they're gone: folders and settings no installed app claims, shortcuts and PATH entries
          pointing nowhere, and startup entries, services and tasks whose programs are missing.{' '}
          <strong>Low</strong> finds are only guesses – check them before you tick them.
        </Note>

        {orphans.error && <Callout message={orphans.error} action="Try again" onAction={orphans.reload} />}
        {(blocked > 0 || orphans.data?.needsAdmin) && (
          <Callout
            message={
              orphans.data?.needsAdmin
                ? 'Scheduled tasks can only be checked with administrator rights, and some finds need them to be removed.'
                : `${plural(blocked, 'find needs', 'finds need')} administrator rights to be removed.`
            }
            action="Restart as administrator"
            onAction={restartAsAdmin}
          />
        )}
        <ProblemList title={`${plural(problems.length, "item wasn't", "items weren't")} removed`} problems={problems} />

        {!orphans.data && orphans.loading && (
          <>
            <SkeletonRows rows={3} />
            <SkeletonRows rows={5} />
          </>
        )}
        {orphans.data && items.length === 0 && (
          <Empty icon="check" title="Nothing left behind">
            Every folder, setting, shortcut and startup entry UWiper looked at belongs to something that's installed.
          </Empty>
        )}
        {items.length > 0 && (
          <>
            <LeftoverReview items={items} selected={selected} onChange={setSelected} canRemove={canRemove} />
            {picked.length > 0 && (
              <div className="text-right text-small text-fg-3">
                {plural(picked.length, 'item', 'items')} selected{bytes > 0 && ` · ${formatBytes(bytes)}`}
              </div>
            )}
          </>
        )}
      </PageBody>

      {confirming && (
        <Dialog
          label="Delete leftovers"
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
          <h2 className="text-h2 font-bold">Delete {plural(picked.length, 'item', 'items')} for good?</h2>
          <p className="mt-2 text-body leading-normal text-fg-3">
            Quarantine is off in Settings, so these can't be put back. Turn it on to keep a copy of everything UWiper removes.
          </p>
        </Dialog>
      )}
    </>
  );
}
