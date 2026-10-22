import { useEffect, useRef, useState, type ReactNode } from 'react';
import { api, errorText } from '../../lib/api';
import { formatBytes, plural } from '../../lib/format';
import type { InstalledApp, Leftover, RemoveReport, RestorePoint } from '../../lib/types';
import { useAppStore } from '../../store/useAppStore';
import { Button, Callout, Dialog, Icon, Note, Pill, SkeletonRows, Spinner, Switch } from '../../ui';
import { LeftoverReview, preselect } from '../leftovers/LeftoverReview';
import { ProblemList } from '../leftovers/ProblemList';
import { AppTile } from './AppTile';

type Step =
  | { name: 'confirm' }
  | { name: 'restore-point' }
  | { name: 'no-restore-point'; message: string }
  | { name: 'running'; error?: string }
  | { name: 'scanning' }
  | { name: 'review'; leftovers: Leftover[]; stillInstalled: boolean }
  | { name: 'removing' }
  | { name: 'done'; report: RemoveReport | null };

function Row({ label, sub, control }: { label: string; sub: string; control: ReactNode }) {
  return (
    <div className="flex items-center gap-4 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="text-label font-medium">{label}</div>
        <div className="mt-0.5 text-small text-fg-3">{sub}</div>
      </div>
      {control}
    </div>
  );
}

/** Runs the app's own uninstaller, then finds and quarantines what it left behind. */
export function UninstallDialog({ app, onClose }: { app: InstalledApp; onClose: () => void }) {
  const env = useAppStore((s) => s.env);
  const safety = useAppStore((s) => s.settings?.safety);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const refresh = useAppStore((s) => s.refresh);
  const afterChange = useAppStore((s) => s.afterChange);
  const go = useAppStore((s) => s.go);

  const elevated = !!env?.elevated;
  const quietAvailable = !!app.quietUninstallString || app.source === 'msi';
  const [step, setStep] = useState<Step>({ name: 'confirm' });
  const [wantRestorePoint, setWantRestorePoint] = useState(!!safety?.restorePoint && elevated);
  const [quiet, setQuiet] = useState(false);
  const [restorePoint, setRestorePoint] = useState<RestorePoint | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const canRemove = (l: Leftover) => elevated || !l.needsAdmin;
  const quarantine = safety?.quarantine ?? true;

  // The watcher and the "It's finished" button may both ask to scan.
  const scanned = useRef(false);
  const scan = async () => {
    if (scanned.current) return;
    scanned.current = true;
    setStep({ name: 'scanning' });
    try {
      const [leftovers, finished] = await Promise.all([api.scanLeftovers(app), api.uninstallFinished(app.id)]);
      setSelected(preselect(leftovers, safety?.preselect ?? 'high', canRemove));
      setStep({ name: 'review', leftovers, stillInstalled: !finished });
    } catch (e) {
      scanned.current = false;
      setStep({ name: 'running', error: errorText(e) });
    }
  };

  const start = async () => {
    if (wantRestorePoint) {
      setStep({ name: 'restore-point' });
      const point = await api.createRestorePoint(app.name).catch((e) => ({ created: false, message: errorText(e) }));
      setRestorePoint(point);
      // What the uninstaller removes isn't in quarantine: without the restore
      // point asked for, nothing could bring it back, so ask first.
      if (!point.created) {
        setStep({ name: 'no-restore-point', message: point.message });
        return;
      }
    }
    run();
  };

  const run = async () => {
    setStep({ name: 'running' });
    try {
      await api.startUninstall(app.id, quiet);
    } catch (e) {
      setStep({ name: 'running', error: errorText(e) });
    }
  };

  // While the uninstaller runs, watch for the app's entry to disappear.
  const waiting = step.name === 'running' && !step.error;
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(() => {
      api.uninstallFinished(app.id).then(
        (done) => {
          if (done) scan();
        },
        () => undefined,
      );
    }, 1500);
    return () => clearInterval(timer);
  }, [waiting, app.id]);

  const remove = async (leftovers: Leftover[]) => {
    const items = leftovers.filter((l) => selected.has(l.id));
    if (items.length === 0) {
      setStep({ name: 'done', report: null });
      return;
    }
    setStep({ name: 'removing' });
    try {
      const report = await api.removeItems(
        `Uninstall ${app.name}`,
        items.map((l) => ({ label: l.label, target: l.target, sizeBytes: l.sizeBytes ?? 0, check: l.check })),
      );
      setStep({ name: 'done', report });
    } catch (e) {
      setStep({ name: 'done', report: { batchId: null, removed: 0, freedBytes: 0, problems: [{ label: app.name, error: errorText(e) }], notes: [] } });
    }
  };

  const close = () => {
    if (step.name !== 'confirm') {
      afterChange();
      refresh('apps');
    }
    onClose();
  };

  const header = (title: string, sub?: string) => (
    <div className="mb-5 flex items-center gap-3.5">
      <AppTile name={app.name} icon={app.icon} large />
      <div className="min-w-0">
        <h2 className="text-h2 font-bold tracking-[-0.01em]">{title}</h2>
        {sub && <div className="mt-0.5 text-meta text-fg-3">{sub}</div>}
      </div>
    </div>
  );

  const progress = (label: string, sub: string) => (
    <div className="flex items-start gap-3 rounded-xl bg-white/4 px-4 py-3.5">
      <Spinner size={16} className="mt-0.5" />
      <div>
        <div className="font-medium">{label}</div>
        <div className="mt-0.5 text-small text-fg-3">{sub}</div>
      </div>
    </div>
  );

  if (step.name === 'confirm') {
    return (
      <Dialog
        label={`Uninstall ${app.name}`}
        onClose={close}
        footer={
          <>
            <Button onClick={close}>Cancel</Button>
            <Button kind="danger" icon="trash" onClick={start}>
              Uninstall
            </Button>
          </>
        }
      >
        {header(`Uninstall ${app.name}?`, [app.publisher, app.version && `version ${app.version}`].filter(Boolean).join(' · '))}
        <p className="text-body leading-normal text-fg-2">
          UWiper runs the app's own uninstaller, then looks for everything it left behind. You check the list before anything is
          {quarantine ? ' moved to quarantine.' : ' deleted.'} What the uninstaller itself removes isn't in quarantine – only a
          restore point can bring that back.
        </p>
        {app.component && (
          <div className="mt-3">
            <Callout message={`${app.component}. Protection for Windows components is off, so UWiper lets you uninstall it – keep the restore point on.`} />
          </div>
        )}
        <div className="mt-3 divide-y divide-line">
          <Row
            label="Create a restore point first"
            sub={elevated ? 'Windows System Restore, in case anything goes wrong' : 'Needs administrator rights'}
            control={<Switch checked={wantRestorePoint} onChange={setWantRestorePoint} disabled={!elevated} label="Create a restore point first" />}
          />
          {quietAvailable && (
            <Row
              label="Quiet uninstall"
              sub="Run the uninstaller without its questions"
              control={<Switch checked={quiet} onChange={setQuiet} label="Quiet uninstall" />}
            />
          )}
        </div>
        {!elevated && (
          <div className="mt-3">
            <Callout
              message="Without administrator rights UWiper can't remove leftovers in Program Files, ProgramData or machine-wide settings."
              action="Restart as administrator"
              onAction={restartAsAdmin}
            />
          </div>
        )}
      </Dialog>
    );
  }

  if (step.name === 'restore-point' || step.name === 'scanning' || step.name === 'removing') {
    const text = {
      'restore-point': ['Creating a restore point…', 'This can take a minute.'],
      scanning: ['Looking for leftovers…', 'Folders, registry, shortcuts, services and scheduled tasks.'],
      removing: [quarantine ? 'Moving to quarantine…' : 'Deleting…', 'Big folders on another drive take a moment to copy.'],
    }[step.name];
    return (
      <Dialog label={`Uninstall ${app.name}`}>
        {header(app.name)}
        {progress(text[0], text[1])}
        {step.name === 'scanning' && (
          <div className="mt-4">
            <SkeletonRows rows={3} tile={false} />
          </div>
        )}
      </Dialog>
    );
  }

  if (step.name === 'no-restore-point') {
    return (
      <Dialog
        label={`Uninstall ${app.name}`}
        onClose={close}
        footer={
          <>
            <Button onClick={close}>Cancel</Button>
            <Button kind="danger" icon="trash" onClick={run}>
              Uninstall without it
            </Button>
          </>
        }
      >
        {header(`No restore point`, app.name)}
        <Callout message={step.message} />
        <p className="mt-3 text-body leading-normal text-fg-2">
          Without a restore point, what {app.name}'s uninstaller removes can't be brought back. Leftovers UWiper removes afterwards
          {quarantine ? ' still go to quarantine.' : ' are deleted for good.'}
        </p>
      </Dialog>
    );
  }

  if (step.name === 'running') {
    return (
      <Dialog
        label={`Uninstall ${app.name}`}
        footer={
          <>
            <Button kind="ghost" onClick={close}>
              Close
            </Button>
            <Button kind={step.error ? 'primary' : 'secondary'} icon="search" onClick={scan}>
              {step.error ? 'Look for leftovers anyway' : "It's finished"}
            </Button>
          </>
        }
      >
        {header(app.name)}
        {step.error ? (
          <Callout message={step.error} />
        ) : (
          progress(`Running ${app.name}'s uninstaller…`, 'Finish it in its own window. UWiper goes on by itself once the app is gone.')
        )}
        {restorePoint && (
          <div className="mt-3 flex items-center gap-2 text-small text-fg-3">
            <Icon name={restorePoint.created ? 'check' : 'info'} size={15} className={restorePoint.created ? 'text-good' : ''} />
            {restorePoint.message}
          </div>
        )}
      </Dialog>
    );
  }

  if (step.name === 'review') {
    const picked = step.leftovers.filter((l) => selected.has(l.id));
    const bytes = picked.reduce((s, l) => s + (l.sizeBytes ?? 0), 0);
    const blocked = step.leftovers.filter((l) => !canRemove(l)).length;
    return (
      <Dialog
        wide
        label={`Leftovers of ${app.name}`}
        onClose={close}
        footer={
          <>
            <span className="mr-auto text-small text-fg-3">
              {plural(picked.length, 'item', 'items')} selected{bytes > 0 && ` · ${formatBytes(bytes)}`}
            </span>
            <Button kind="ghost" onClick={close}>
              Leave them
            </Button>
            <Button kind={quarantine ? 'primary' : 'danger'} icon={quarantine ? 'archive' : 'trash'} disabled={picked.length === 0} onClick={() => remove(step.leftovers)}>
              {quarantine ? 'Move to quarantine' : 'Delete for good'}
            </Button>
          </>
        }
      >
        {header(
          step.leftovers.length ? `${plural(step.leftovers.length, 'leftover', 'leftovers')} found` : 'Nothing left behind',
          app.name,
        )}
        <div className="flex flex-col gap-3">
          {step.stillInstalled && (
            <Callout message={`${app.name} still looks installed – the uninstaller may have been cancelled. The list includes the app itself.`} />
          )}
          {blocked > 0 && (
            <Callout
              message={`${plural(blocked, 'item needs', 'items need')} administrator rights. Restarting UWiper forgets this list.`}
              action="Restart as administrator"
              onAction={restartAsAdmin}
            />
          )}
          {step.leftovers.length === 0 ? (
            <Note icon="check">The uninstaller cleaned up after itself – no files, settings, services or tasks were left.</Note>
          ) : (
            <>
              <LeftoverReview items={step.leftovers} selected={selected} onChange={setSelected} canRemove={canRemove} />
              <Note>
                <strong>High</strong> – certainly the app's, ticked for you. <strong>Medium</strong> – named after it: look before ticking.{' '}
                <strong>Low</strong> – may be shared or still in use; check before removing.
              </Note>
            </>
          )}
        </div>
      </Dialog>
    );
  }

  // Done
  const report = step.report;
  return (
    <Dialog
      label={`Uninstall ${app.name}`}
      onClose={close}
      footer={
        <>
          {report?.batchId && (
            <Button
              kind="ghost"
              onClick={() => {
                close();
                go('quarantine');
              }}
            >
              Open quarantine
            </Button>
          )}
          <Button kind="primary" onClick={close}>
            Done
          </Button>
        </>
      }
    >
      <div className="mb-4 flex flex-col items-center gap-2 text-center">
        <span className="grid size-14 place-items-center rounded-full bg-good/14 text-good-fg">
          <Icon name="check" size={28} stroke={2.4} />
        </span>
        <h2 className="text-h2 font-bold">{app.name} is cleaned up</h2>
        <p className="max-w-[400px] text-body leading-normal text-fg-3">
          {!report || report.removed === 0
            ? 'Nothing was removed besides what the uninstaller took care of.'
            : report.batchId
              ? `${plural(report.removed, 'item', 'items')}${report.freedBytes ? ` (${formatBytes(report.freedBytes)})` : ''} moved to quarantine – restorable for ${safety?.keepDays ?? 30} days.`
              : `${plural(report.removed, 'item', 'items')}${report.freedBytes ? ` (${formatBytes(report.freedBytes)})` : ''} deleted.`}
        </p>
        {restorePoint && <Pill tone={restorePoint.created ? 'good' : 'dim'}>{restorePoint.message}</Pill>}
      </div>
      {report && (
        <div className="flex flex-col gap-3">
          <ProblemList title={`${plural(report.problems.length, "item wasn't", "items weren't")} removed`} problems={report.problems} />
          {report.notes.map((n) => (
            <Note key={n} icon="warning">
              {n}
            </Note>
          ))}
        </div>
      )}
    </Dialog>
  );
}
