import { useCallback, useEffect, useState } from 'react';
import { TraceCard } from '../components/installs/TraceCard';
import { TrackCard } from '../components/installs/TrackCard';
import { PageBody, PageHeader } from '../components/layout/Page';
import { LeftoverReview, preselect } from '../components/leftovers/LeftoverReview';
import { ProblemList } from '../components/leftovers/ProblemList';
import { api, errorText } from '../lib/api';
import { formatBytes, plural } from '../lib/format';
import type { Leftover, Problem, TraceView } from '../lib/types';
import { useResource } from '../store/hooks';
import { useAppStore } from '../store/useAppStore';
import { Button, Callout, Dialog, Empty, GroupTitle, Note, SkeletonRows, Spinner } from '../ui';

/** What a trace recorded and is still there, to review and remove. */
function CleanUpDialog({ trace, onClose }: { trace: TraceView; onClose: () => void }) {
  const env = useAppStore((s) => s.env);
  const safety = useAppStore((s) => s.settings?.safety);
  const notify = useAppStore((s) => s.notify);
  const afterChange = useAppStore((s) => s.afterChange);
  const [items, setItems] = useState<Leftover[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [removing, setRemoving] = useState(false);
  const [problems, setProblems] = useState<Problem[]>([]);
  const elevated = !!env?.elevated;
  const quarantine = safety?.quarantine ?? true;
  const canRemove = (l: Leftover) => elevated || !l.needsAdmin;

  useEffect(() => {
    api.traceLeftovers(trace.id).then(
      (found) => {
        setItems(found);
        setSelected(preselect(found, safety?.preselect ?? 'high', (l) => elevated || !l.needsAdmin));
      },
      (e) => {
        notify(errorText(e));
        onClose();
      },
    );
  }, [trace.id, safety?.preselect, elevated, notify, onClose]);

  const picked = (items ?? []).filter((l) => selected.has(l.id));
  const bytes = picked.reduce((s, l) => s + (l.sizeBytes ?? 0), 0);

  const remove = async () => {
    setRemoving(true);
    try {
      const report = await api.removeItems(
        trace.name,
        picked.map((l) => ({ label: l.label, target: l.target, sizeBytes: l.sizeBytes ?? 0, check: l.check })),
      );
      setProblems(report.problems);
      if (report.removed > 0) {
        const what = `${plural(report.removed, 'item', 'items')}${report.freedBytes ? ` (${formatBytes(report.freedBytes)})` : ''}`;
        notify(report.batchId ? `${what} moved to quarantine` : `${what} deleted`);
      }
      if (report.notes.length) notify(report.notes.join(' '));
      afterChange();
      if (report.problems.length === 0) onClose();
    } catch (e) {
      notify(errorText(e));
    }
    setRemoving(false);
  };

  return (
    <Dialog
      wide
      label={`Remove what «${trace.name}» left`}
      onClose={onClose}
      footer={
        <>
          {picked.length > 0 && (
            <span className="mr-auto text-small text-fg-3">
              {plural(picked.length, 'item', 'items')} selected{bytes > 0 && ` · ${formatBytes(bytes)}`}
            </span>
          )}
          <Button onClick={onClose}>Cancel</Button>
          <Button
            kind={quarantine ? 'primary' : 'danger'}
            icon={quarantine ? 'archive' : 'trash'}
            disabled={picked.length === 0}
            busy={removing}
            onClick={remove}
          >
            {quarantine ? `Move ${picked.length} to quarantine` : `Delete ${picked.length} for good`}
          </Button>
        </>
      }
    >
      <h2 className="text-h2 font-bold">What «{trace.name}» left</h2>
      <p className="mb-4 mt-2 text-body leading-normal text-fg-3">
        Everything its installer added that's still on this PC. Folders of apps that are installed now are left out.
      </p>
      <ProblemList title={`${plural(problems.length, "item wasn't", "items weren't")} removed`} problems={problems} />
      {items === null ? (
        <div className="flex items-center gap-3 py-6 text-body text-fg-2">
          <Spinner size={18} />
          Checking what's still there…
        </div>
      ) : items.length === 0 ? (
        <Empty icon="check" title="Nothing left">
          Everything its installer added is gone already.
        </Empty>
      ) : (
        <LeftoverReview items={items} selected={selected} onChange={setSelected} canRemove={canRemove} />
      )}
    </Dialog>
  );
}

export function Installs() {
  const go = useAppStore((s) => s.go);
  const notify = useAppStore((s) => s.notify);
  const refresh = useAppStore((s) => s.refresh);
  const tracking = useResource('tracking');
  const traces = useResource('traces');
  const [flow, setFlow] = useState(false);
  const [cleaning, setCleaning] = useState<TraceView | null>(null);
  const [forgetting, setForgetting] = useState<TraceView | null>(null);

  const showFlow = flow || !!tracking.data?.active;
  const closeCleanUp = useCallback(() => setCleaning(null), []);
  const list = traces.data ?? [];

  const forget = async (trace: TraceView) => {
    setForgetting(null);
    try {
      await api.deleteTrace(trace.id);
      refresh('traces');
    } catch (e) {
      notify(errorText(e));
    }
  };

  const track = (
    <Button kind="primary" icon="install" onClick={() => setFlow(true)}>
      Track an install
    </Button>
  );

  return (
    <>
      <PageHeader title="Installs" subtitle="Track what an installer adds, so uninstalling the app later removes all of it">
        {!showFlow && list.length > 0 && track}
      </PageHeader>
      <PageBody>
        <Note icon="lock">
          UWiper compares two snapshots of your PC, before and after installing. They hold only the names of files, folders, registry
          keys and services – never what's in them – and stay in UWiper's folder.
        </Note>

        {showFlow && <TrackCard onClose={() => setFlow(false)} />}

        {traces.error && <Callout message={traces.error} action="Try again" onAction={traces.reload} />}
        {traces.loading && !traces.data && <SkeletonRows rows={3} />}
        {traces.data && list.length === 0 && !showFlow && (
          <Empty icon="install" title="No tracked installs yet" action={track}>
            Track the next app you install: UWiper notes the folders, registry keys, services, tasks and shortcuts its installer adds,
            and uninstalling it later removes every one of them.
          </Empty>
        )}
        {list.length > 0 && (
          <div>
            <GroupTitle aside={plural(list.length, 'install', 'installs')}>Tracked installs</GroupTitle>
            <div className="flex flex-col gap-2.5">
              {list.map((trace) => (
                <TraceCard
                  key={trace.id}
                  trace={trace}
                  onUninstall={() => trace.appId && go('apps', trace.appId)}
                  onCleanUp={() => setCleaning(trace)}
                  onDelete={() => setForgetting(trace)}
                />
              ))}
            </div>
          </div>
        )}
      </PageBody>

      {cleaning && <CleanUpDialog trace={cleaning} onClose={closeCleanUp} />}
      {forgetting && (
        <Dialog
          label="Forget this trace"
          onClose={() => setForgetting(null)}
          footer={
            <>
              <Button onClick={() => setForgetting(null)}>Cancel</Button>
              <Button kind="danger" onClick={() => forget(forgetting)}>
                Forget
              </Button>
            </>
          }
        >
          <h2 className="text-h2 font-bold">Forget the trace of «{forgetting.name}»?</h2>
          <p className="mt-2 text-body leading-normal text-fg-3">
            Nothing on your PC changes. Uninstalling the app later falls back to the usual leftover scan, which may miss a few things.
          </p>
        </Dialog>
      )}
    </>
  );
}
