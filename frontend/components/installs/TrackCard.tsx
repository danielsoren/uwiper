import { useEffect, useState } from 'react';
import { api, errorText, pickInstaller } from '../../lib/api';
import { cn } from '../../lib/cn';
import { formatBytes, plural } from '../../lib/format';
import type { Leftover, TraceDraft } from '../../lib/types';
import { useAppStore } from '../../store/useAppStore';
import { Button, Callout, Card, Empty, Icon, Note, Select, Spinner } from '../../ui';
import { LeftoverReview } from '../leftovers/LeftoverReview';

type Step = 'intro' | 'snapshot' | 'installing' | 'comparing' | 'review';

const STEPS = ['Snapshot', 'Install', 'Review'];
const STEP_INDEX: Record<Step, number> = { intro: 0, snapshot: 0, installing: 1, comparing: 2, review: 2 };
const NO_APP = '';

function Steps({ step }: { step: Step }) {
  const at = STEP_INDEX[step];
  return (
    <ol className="flex flex-wrap items-center gap-2 text-small font-medium">
      {STEPS.map((label, i) => (
        <li key={label} className="flex items-center gap-2">
          {i > 0 && <span className="h-px w-5 bg-line-2" />}
          <span
            className={cn(
              'grid size-5 place-items-center rounded-full text-micro font-semibold leading-[normal] tabular-nums',
              i < at ? 'bg-good/18 text-good' : i === at ? 'bg-accent text-white' : 'bg-white/8 text-fg-4',
            )}
          >
            {i < at ? <Icon name="check" size={12} stroke={3} /> : i + 1}
          </span>
          <span className={i === at ? 'text-fg' : 'text-fg-3'}>{label}</span>
        </li>
      ))}
    </ol>
  );
}

function Working({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-3 py-6 text-body text-fg-2">
      <Spinner size={18} />
      {text}
    </div>
  );
}

/** "Oct 22, 01:34 (2 h 10 min ago)" */
function since(unix: number): string {
  const at = new Date(unix * 1000);
  const minutes = Math.max(0, Math.round((Date.now() / 1000 - unix) / 60));
  const ago = minutes < 1 ? 'just now' : minutes < 60 ? `${minutes} min ago` : `${Math.floor(minutes / 60)} h ${minutes % 60} min ago`;
  return `${at.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })}, ${at.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })} (${ago})`;
}

/** A draft item in the shape the leftover review shows. */
const asLeftover = (i: TraceDraft['items'][number]): Leftover => ({
  id: i.id,
  kind: i.kind,
  label: i.label,
  reason: i.sure
    ? 'New'
    : ['service', 'task', 'startup'].includes(i.kind)
      ? "New, but it runs something the installer didn't add"
      : i.kind === 'driver' || i.kind === 'environment'
        ? 'New – Windows or another app may have added it'
        : "New, but nothing ties it to this install – another app may have made it",
  confidence: i.sure ? 'high' : 'medium',
  sizeBytes: i.sizeBytes,
  modified: null,
  needsAdmin: false,
  target: i.target,
});

/** Tracking an install, from the first snapshot to the saved trace. */
export function TrackCard({ onClose }: { onClose: () => void }) {
  const env = useAppStore((s) => s.env);
  const tracking = useAppStore((s) => s.res.tracking.data);
  const load = useAppStore((s) => s.load);
  const update = useAppStore((s) => s.update);
  const refresh = useAppStore((s) => s.refresh);
  const notify = useAppStore((s) => s.notify);
  const restartAsAdmin = useAppStore((s) => s.restartAsAdmin);
  const [step, setStep] = useState<Step>(tracking?.active ? 'installing' : 'intro');
  const [draft, setDraft] = useState<TraceDraft | null>(null);
  const [keep, setKeep] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(false);

  // While installing, keep an eye on the installer UWiper started.
  useEffect(() => {
    if (step !== 'installing') return;
    const timer = setInterval(() => load('tracking', true), 2000);
    return () => clearInterval(timer);
  }, [step, load]);

  // Tracking that was started earlier (before a restart, say) picks up here.
  useEffect(() => {
    if (tracking?.active && step === 'intro') setStep('installing');
  }, [tracking?.active, step]);

  const elevated = !!env?.elevated;
  const mismatch = !!tracking?.active && tracking.elevated !== elevated;

  const show = (found: TraceDraft) => {
    setDraft(found);
    setKeep(new Set(found.items.filter((i) => i.sure).map((i) => i.id)));
  };

  const launch = async (path: string) => {
    try {
      await api.launchInstaller(path);
    } catch (e) {
      notify(errorText(e));
    }
    load('tracking', true);
  };

  const start = async (withInstaller: boolean) => {
    const installer = withInstaller ? await pickInstaller() : null;
    if (withInstaller && !installer) return;
    setStep('snapshot');
    try {
      await api.startTracking();
    } catch (e) {
      notify(errorText(e));
      setStep('intro');
      return;
    }
    if (installer) await launch(installer);
    load('tracking', true);
    setStep('installing');
  };

  const finish = async () => {
    setStep('comparing');
    try {
      show(await api.finishTracking());
      setStep('review');
    } catch (e) {
      notify(errorText(e));
      setStep('installing');
    }
  };

  const choose = async (appId: string) => {
    setBusy(true);
    try {
      show(await api.chooseTraceApp(appId === NO_APP ? null : appId));
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(false);
  };

  /** Tracking is over: the card goes away without waiting for the next status. */
  const over = () => {
    setDone(true);
    update('tracking', (t) => ({ ...t, active: false, startedAt: null, installer: null }));
    refresh('tracking');
    onClose();
  };

  const cancel = async () => {
    setBusy(true);
    try {
      await api.cancelTracking();
      over();
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(false);
  };

  const save = async () => {
    if (!draft || done) return;
    setBusy(true);
    try {
      await api.saveTrace([...keep]);
      notify(`Saved what «${draft.name}» added`);
      refresh('traces', 'apps');
      over();
    } catch (e) {
      notify(errorText(e));
    }
    setBusy(false);
  };

  const kept = draft?.items.filter((i) => keep.has(i.id)) ?? [];
  const keptBytes = kept.reduce((s, i) => s + (i.sizeBytes ?? 0), 0);
  const long = !!tracking?.startedAt && Date.now() / 1000 - tracking.startedAt > 3 * 3600;

  return (
    <Card className="flex flex-col gap-4 px-[22px] py-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className="text-title font-semibold">Track an install</h2>
        <Steps step={step} />
      </div>

      {step === 'intro' && (
        <>
          <p className="max-w-[640px] text-body leading-normal text-fg-2">
            UWiper takes a snapshot now, you install the app, and UWiper looks at what's new. Close other apps first – anything they
            add in the meantime could look like the installer's.
          </p>
          {!elevated && <Note>Scheduled tasks are only seen with administrator rights – restart UWiper as administrator first to include them.</Note>}
          <div className="flex flex-wrap gap-2.5">
            <Button kind="primary" icon="install" onClick={() => start(true)}>
              Choose installer…
            </Button>
            <Button onClick={() => start(false)}>I'll run it myself</Button>
            <Button kind="ghost" onClick={onClose}>
              Cancel
            </Button>
          </div>
        </>
      )}

      {step === 'snapshot' && <Working text="Taking a snapshot of your PC…" />}

      {step === 'installing' && (
        <>
          {mismatch && (
            <Callout
              message={
                tracking?.elevated
                  ? 'Tracking was started with administrator rights. Restart UWiper as administrator to finish it – or stop tracking.'
                  : 'Tracking was started without administrator rights. Open UWiper normally to finish it – or stop tracking.'
              }
              action={tracking?.elevated ? 'Restart as administrator' : undefined}
              onAction={tracking?.elevated ? restartAsAdmin : undefined}
            />
          )}
          <div className="flex items-start gap-3">
            <span
              className={cn(
                'mt-0.5 grid size-8 flex-none place-items-center rounded-full',
                tracking?.installerRunning ? 'bg-accent/20 text-accent-soft' : 'bg-white/8 text-fg-3',
              )}
            >
              {tracking?.installerRunning ? <Spinner size={15} /> : <Icon name="install" size={16} />}
            </span>
            <div>
              <div className="text-label font-semibold">
                {!tracking?.installer
                  ? 'Run the installer now'
                  : !tracking.installerFollowed
                    ? `Install «${tracking.installer}»`
                    : tracking.installerRunning
                      ? `«${tracking.installer}» is running`
                      : `«${tracking.installer}» has closed`}
              </div>
              <p className="mt-1 max-w-[680px] text-small leading-normal text-fg-3">
                {tracking?.installer && !tracking.installerFollowed
                  ? "UWiper can't tell whether it's still running – click Done once it has finished."
                  : tracking?.installerRunning
                    ? 'Go through it the way you normally would. If it opens the app at the end, that first start is part of the trace too.'
                    : 'When the installer has finished – and the app has opened once, if you like – click Done.'}
                {tracking?.startedAt && ` Tracking since ${since(tracking.startedAt)}. You can close UWiper or restart Windows in between.`}
              </p>
            </div>
          </div>
          {long && (
            <Note icon="warning">
              Tracking has been on for a long time, so other apps had plenty of time to add things too. Nothing will be ticked for you – look
              through the list, or stop tracking and start again right before installing.
            </Note>
          )}
          <div className="flex flex-wrap gap-2.5">
            <Button kind="primary" icon="check" disabled={mismatch} onClick={finish}>
              Done installing
            </Button>
            {!tracking?.installer && (
              <Button
                icon="install"
                onClick={async () => {
                  const path = await pickInstaller();
                  if (path) await launch(path);
                }}
              >
                Choose installer…
              </Button>
            )}
            <Button kind="ghost" busy={busy} onClick={cancel}>
              Stop tracking
            </Button>
          </div>
        </>
      )}

      {step === 'comparing' && <Working text="Looking for what the installer added…" />}

      {step === 'review' && draft && (
        <>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0">
              <div className="text-label font-semibold">What «{draft.name}» added</div>
              <div className="mt-0.5 text-small text-fg-3">
                {draft.appId
                  ? [draft.publisher, draft.version && `version ${draft.version}`].filter(Boolean).join(' · ')
                  : 'Not linked to an installed app – the trace is named after the installer'}
              </div>
            </div>
            {draft.candidates.length > 0 && (
              <label className="inline-flex items-center gap-2 text-meta text-fg-2">
                Belongs to
                <Select
                  label="The app this install was for"
                  value={draft.appId ?? NO_APP}
                  disabled={busy}
                  options={[...draft.candidates.map((c) => ({ value: c.id, label: c.name })), { value: NO_APP, label: 'No app' }]}
                  onChange={choose}
                  className="max-w-[260px]"
                />
              </label>
            )}
          </div>
          {draft.updated && <Note>«{draft.name}» was already installed, so this was an update: only what it added is here.</Note>}
          {draft.alsoInstalled.length > 0 && (
            <Note>
              Also installed: {draft.alsoInstalled.map((n) => `«${n}»`).join(', ')}. What's in {draft.alsoInstalled.length > 1 ? 'their folders' : 'its folder'}{' '}
              stays out of this trace – other apps may need {draft.alsoInstalled.length > 1 ? 'them' : 'it'}.
            </Note>
          )}
          {draft.longWindow && (
            <Note icon="warning">Tracking ran for hours, so nothing is ticked – other apps had time to add things too. Tick what's the app's.</Note>
          )}
          {!draft.tasksChecked && <Note>Scheduled tasks weren't checked – UWiper needs administrator rights to see them.</Note>}
          {draft.items.length === 0 ? (
            <Empty icon="check" title="Nothing new">
              UWiper didn't find anything the installer added. If it's still running, go back and click Done once it has finished.
            </Empty>
          ) : (
            <>
              <p className="text-small leading-normal text-fg-3">
                Untick anything that isn't the app's. <strong className="font-semibold text-fg-2">Medium</strong> items might be another app's,
                so they're left out unless you tick them.
              </p>
              <LeftoverReview items={draft.items.map(asLeftover)} selected={keep} onChange={setKeep} canRemove={() => true} />
            </>
          )}
          <div className="flex flex-wrap items-center justify-end gap-2.5">
            {kept.length > 0 && (
              <span className="mr-auto text-small text-fg-3">
                {plural(kept.length, 'item', 'items')} kept{keptBytes > 0 && ` · ${formatBytes(keptBytes)}`}
              </span>
            )}
            {draft.items.length === 0 && <Button onClick={() => setStep('installing')}>Back</Button>}
            <Button kind="ghost" disabled={busy || done} onClick={cancel}>
              Discard
            </Button>
            <Button kind="primary" icon="check" disabled={kept.length === 0 || done} busy={busy} onClick={save}>
              Save trace
            </Button>
          </div>
        </>
      )}
    </Card>
  );
}
